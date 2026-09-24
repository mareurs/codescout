I found 14 problems in the review scope. Four of them change a stated basis for a conclusion or for the next step:

- **Item 1:** the JevK5 "lead for Stage 3" rests on a miscount.
- **Item 3:** the "12 of 12 runs" figure behind the form-2b ruling is false.
- **Item 4:** a published "reproduces exactly" statement is wrong for RTD-9.
- **Item 5:** the local pre-registration's planned Score B names a checker that fails its gate on the clean channel.

None of them reverses a registered ship verdict. Most published numbers do reconcile with the committed rows (list at the end). I ran read-only Python and grep only; scratch output is in `/tmp/rv-s1-claude-cold-1/`.

## Findings

**1. The L0 "gold ranks 1st or 2nd on 4 of 5 violation texts" claim is false. It is 3 of 5, and the handoff builds Stage 3's lead on it.**
- **Evidence:** `rule-tell-scoring-2026-09-23.md:561` and `:617`.
  - `l0-gate.jsonl` has no case field. I split it by write order (22 rows per text, gate order) and confirmed each block's fired set matches `l0-gate.txt` exactly.
  - Gold rank is: semicolon 1, member 2, cannot 1, **sessionid 4**, and **contradiction 13** (the table at `:569` says 12).
  - Only semicolon and member are both ranked ≤2 **and** strictly above the clean-text maximum.
  - For `cannot`, gold p = 0.600599, which exactly equals clean-1's value. The "+0.00 margin" row is a tie, not a margin.
- **Impact:** the only stated evidence that per-rule calibration is the lead for Stage 3 is overstated.

**2. JevK5's `noul` outputs sit on a coarse grid, and the 0.5 threshold is one of its points, so several L0 results are tie artefacts.**
- **Evidence:** 220 rows hold only 41 distinct values, all multiples of about 0.0816 in log-odds. 8 rows are exactly 0.5 and count as fires, because `phase1-local-l0.py:56` only rejects `p < THRESHOLD`.
  - The `sessionid` gate pass rests on `d_sessionid` = 0.500000, tied with three other rules.
  - With a strict `p > 0.5`, the gate would be 3/10, not 4/10.
- **Impact:** the registered prediction (≤ 6/10) still holds. But the per-text ranks, margins and the one sessionid "pass" are not real signal. Any Stage-3 threshold or ranking on this output needs a tie policy.

**3. "clean-2 fires `member_vs_population` on every run of both models on both channels, 12 of 12 runs" is false.**
- **Evidence:** the claim is at scoring doc `:341` and is repeated at `phase1-local-classifier-preregistration.md:172`.
  - `p1s-H0clean-gate.txt` shows clean-2 firing `[member_vs_population, question_asked]`, then `[question_asked]`, then `[d_history]`: **1 of 3 runs**.
  - Form 2 total is 7 of 9 runs (H0 3/3, H0-clean 1/3, S0 3/3). Sonnet never ran on the dirty channel. Adding form 1 gives 8 of 12.
- **Impact:** this was the stated evidence that the fixture and the spec systematically disagree, and the operator's ruling that produced form 2b rested on it. On the clean channel, Haiku's clean-2 failures are spread across other rules.

**4. "The claim-bound arms reproduce on the clean channel exactly for RTD-9, RTD-10 and RTD-3" is false for RTD-9.**
- **Evidence:** scoring doc `:424`. The same table at `:412` shows RTD-9 1b at `4/9 [3/9]`. `fsc-rtd9.txt` has 1b 3/9, and `e2s-score-rtd9.txt` has 1b 4/9.
- **Impact:** a published confirmation ("the contamination changed no comparison") is wrong about the one row where the channel moved a count.

**5. The local route's Stage 4 Score B, which is the planned next step, is registered with a checker that cannot run and with superseded comparison rows.**
- **Evidence:** `phase1-local-classifier-preregistration.md:99` names `rtd8`. `rtd8` fails its gate on the clean channel (`e2s-score-rtd8.txt`: recorded fixture NO ×3, "GATE FAILED").
  - `:28` and the ship rule compare against contaminated-channel rows: RTD-8 arm 0 5/10 and 1b 0/10, where the clean `rtd8c` figures are 9/10 and 2/10.
  - Stage 4 still names an 8-text gate (`:88`), but the gate is now 10 texts.
  - Only the S0 amendment re-pointed these. Stage 4 was never amended.
- **Impact:** run as written, Stage 4 Score B would either stop at the RTD-8 gate or compare across judge channels.

**6. Several reported numbers have no committed evidence, and the README mislabels one file.**
- **Evidence:**
  - `score-rtd910.txt` contains only usage-cap crash tracebacks (`grep` finds no score table), yet the README lists it as "the original scores". So the API-judge arm-0 rates cited at scoring doc `:161` and pre-reg `:187`/`:210` (RTD-9 4/10, RTD-10 5/10) are unevidenced.
  - No committed file supports any of these:
    - the Jev `noul` gate table (`:95-101`);
    - the phase-1A gates (Jev 6/6, Haiku 4/6);
    - the 15-fixture mutation gates for the native and reasoned forms;
    - Jev `choice` picking `none` on 10/10 fork drafts (`:198`).
- **Impact:** the README's "this directory is the evidence" does not hold for these results.

**7. "The judge channel moves rates on identical replays" is confounded for RTD-10.**
- **Evidence:** scoring doc `:161` compares API 5/10 with subscription 9/10. The subscription score used the second RTD-10 wording (pre-reg `:224`); the API score used the first.
  - On the clean channel with the second wording, arm 0 is 9/10 (`api-score-rtd10.txt`), identical to the contaminated channel.
- **Impact:** for RTD-10 the shift cannot be attributed to the channel. It is channel and wording together. The "channel effect as large as the effects under test" lesson is overstated.

**8. "At RTD-3, S0's reminder halves the violation rate" credits the reminder with a pipeline-level difference.**
- **Evidence:** scoring doc `:423`. In `fork-e2s-rtd3.jsonl`, 6 of 10 e2s forks had `injected: None`, making them arm-0 replicas on a different day and account. There is no per-row breakdown.
- **Impact:** the drop from 8/10 to 4/10 may come mostly from uninjected forks. The attribution is unsupported.

**9. RTD-10's "fragile pass" bound mis-describes the unobservable forks, and the observable is asymmetric within the same arm.**
- **Evidence:** in `fork-e2s-dp1.jsonl`, the 3 forks that `rtd9`/`rtd10` cannot observe are runs 1, 2 and 5. Runs 2 and 5 made **no tool call**; they replied in prose. The doc (`:422`) says they "took a first action other than the doc write".
  - Run 2's reply explicitly withdraws the "cannot arise" claim.
  - `rtd8c` observes and scores those same prose rows. `rtd9`/`rtd10` drop them because their observable requires a `doc` tool call.
- **Impact:** this is the same recording filter that the RTD-3 `rtd3r` re-registration fixed, and it recurs here unnoted. The "all 3 as violations" worst case is too pessimistic for at least run 2.

**10. "s0 is not below arm 0" is reported as holding where the data show it slightly below.**
- **Evidence:**
  - `:192` says "for all three rules", but RTD-9 s0 is 5/8 (0.625) against arm 0 at 7/10 (`fsc-rtd9.txt`).
  - `:456` says "Holds" for 7/8 against 9/10 (0.875 < 0.90). It gets there by also citing the API route's 8/10, which the fork registration (pre-reg `:240`) says not to use.
- **Impact:** the differences are small, but the prediction is reported as holding when it literally does not.

**11. The family-recall "upper bound" of 5/17 is not an upper bound.**
- **Evidence:** scoring doc `:528`. In form 3, gold fired on 3 positives and a wrong rule fired on 4 (RTD-4, 9, 11, 20). Any grouping that absorbs all four confusions gives 7/17.
- **Impact:** the verdict does not change (7/17 < 0.5), but the stated ceiling and "no fairer grouping can do better" are wrong.

**12. `scripts/phase2-score-dp1.py` defaults to the known-contaminated judge channel and has no refusal.**
- **Evidence:** `:212-213` defaults to `JUDGE_CONFIG_DIR` = `~/.claude-kat`. Unlike `phase1-span-selector.py:451-462`, it has no clean-channel check.
- **Impact:** the handoff (`:622`) warns about this, but the script is the one the future Score B runs use. Forgetting the env var silently reproduces the contamination.

**13. `l0-gate.jsonl` rows carry no text or case id.**
- **Evidence:** keys are only `rule`, `noul`, `verdict`, `claim` (`phase1-local-l0.py:55-72`). Attribution depends on write order, which is valid only because the pool is 1. The log header also says "subscription judge" for a local model.
- **Impact:** the "every rule's `noul` probability" data needs out-of-band knowledge to interpret.

**14. Minor count slips in the phase-1 tables.** Each is checked against the committed logs:
- Form-1 clean-1 fired 4–5 rules, not 3–5 (`:293`).
- Form-1 `cannot` fired +11 to 13 extra rules, not +12 (`:297`).
- Form-1 `contradiction` fired +11, not +10 (`:298`).
- S0 `contradiction` fired +1 to 4 extras, not +0 to 4 (`:329`, `:352`).
- The pre-reg's "4–13 unrelated rules" is really 3–13 (`:316`).
- Form 3 "removes one corpus false positive" (`:515`): it is 3 fewer negative fires (2/21 against 5/21).
- "Jev ranked the correct rule first (0.40)" (`:135`) holds in 2 of 3 runs; one run picked `none`.

## Checks that came out clean

- **API-route re-score:** RTD-8 `rtd8c` rows are 10, 9, 9, 0/9, 0 (arms 0, 2, 1a, 1b, 3). RTD-9 and RTD-10 rows, and both ship-rule differences for each rule, also match.
- **Earlier RTD-8 API score** in `phase2-dp1-score.txt`: 8, 8, 0/9, 9, 0.
- **Fork route, contaminated channel:** RTD-8, 9 and 10 (arms 0, s0, s1a, s1b, 1b) and route validity 5/10 match the `fsc-*.txt` files.
- **RTD-3:**
  - Doc-write observable (3/3, 3/3, 0/1, 0/5) and `rtd3r` (8, 8, 9, 0, 0 of 10).
  - Exploratory re-score (5, 8, 1, 0).
  - "3/6/3 non-doc first actions" and the ≥ 5/7 and ≥ 2/4 bounds.
  - All 50 `rtd3r` forks observable; no row has a tool call without prose.
  - The 7 old 3-1b no-tool replies, which do each retract the claim.
- **`rtd8c` fork re-score:** 9/10, 2/10, 7/8, 7/9, 1/10 and e2s 9/9. The e2s build contents, the claims bound in runs 1 and 3, and "0 of 2 bindings used `contradiction`" all check out. The one `rtd8c`-unobservable e2s fork is run 1.
- **Score B:** RTD-9 e2s 3/7, RTD-10 2/7, RTD-3 4/10; margins 0.27, 0.51, 0.40; "6 of 10 RTD-3 drafts produced no injection".
- **Score A, forms 2b and 3:**
  - Full rows: 924 each, 22 per text, 0 errored.
  - Each bucket's cells, and the 13/1/3 and 10/4/3 splits.
  - The new negative fires.
  - Wilson interval for 3/17 ≈ [0.06, 0.41].
  - Localisation 3/3.
- **Phase-1 gate logs:** gate2, H0-clean and S0 at 4/8, 4/8 and 7/8; the span gates; S0b 10/10; form 3 at 10/10 with one clean-4 fire; L0 at 4/10 with span 3/3.
- **L0 probability claims:** every `noul` value lies between 0.086 and 0.858, and `cannot_happen` sits at 0.44–0.60 and fires on 9 of 10 texts.
- **Phase 1A:** Jev 12/51, 27/51, 30/63; Haiku 12/85, 30/85, 1/105; 42 of 42 distributions differ; 33 stable top picks; mean P(`none`) 0.27 against 0.24; the RTD-10, RTD-15 and RTD-8 pair values.
- **Phase 0:**
  - Reasoned form: all 2,850 rows are n = 10 per task, and every table cell matches (positives 1.0, 0.9, 0.8, 0.0; near-miss and full-shape columns; 1 of 8 near-misses; RTD-10 at 9 of 12; off-diagonal 0.33, 0.30, 0.22, 0.08, 0.07).
  - `CTL9-4` reasoning cites the date in 10 of 10 runs.
  - Rubric against native: 44 of 44 disagreements run one way, 22 YES verdicts below 1.0, and one "pushes toward NO" at 0.7.
  - Structured path: 72% of rows (205/285) have no reasoning.
- **Code:** in `report_corpus`, the completeness check works (`p1s-offline.txt` refuses both a 1-row and a duplicated sweep). The form-3 string derivation asserts correctly. The `phase1-local-l0.py` substitution of `judge_rule` reaches `sweep` through module globals, as intended.

`★ Insight ─────────────────────────────────────`
- The quantised `noul` scores (item 2) come from the model, not the harness. The recurring values look like probabilities computed from low-precision (bf16) numbers. So "max |Δp| = 0" in the determinism test partly just shows how coarse the output is, and any per-rule threshold fitted later sits on these same fixed points.
- Items 4 and 9 are the campaign's own "recording filter" and "publish the denominator" laws turning up in its own write-up: a confirmation stated without re-checking it against the table beside it, and an observable (a doc write is required) that silently drops prose-only retractions for two of the three checkers scoring the same row.
- Most arithmetic reconciled. The defects cluster in prose summaries of the rows (ranks, "every run", "exactly"), where no script recomputes the claim.
`─────────────────────────────────────────────────`