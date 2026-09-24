I reviewed everything in scope. The raw rows back most of the headline numbers. The defects are in how some results are read, in claims the data contradicts, and in next steps that rest on figures that don't hold. Scratch scripts are in `/tmp/rv-s1-claude-cold-2/`. Nothing in the checkout was modified.

## Findings (most severe first)

**1. The failed H2 prediction is never reported.**
- (a) The pre-registration predicted that injection **fails** for RTD-9 and RTD-10. Both shipped, and the scoring doc mentions the prediction only where it held.
- (b) `rule-injection-timing-preregistration.md:43-46` registers RTD-9 as "fails — needs a fact, not a rule" and RTD-10 as "fails — needs a gate before the claim". It also says: "If it lands scrambled, the predictor is dead."
  - The results: RTD-9 arm 1b 0/9, RTD-10 arm 1b 0/10, both ship (`api-score-rtd9.txt`, `api-score-rtd10.txt`).
  - `grep -n partition rule-tell-scoring-2026-09-23.md` finds only line 130: "H2's predicted partition held for this tell [RTD-8]". Line 164 then says "The pattern holds across all three rules".
- (c) The partition got 2 of 4 wrong, so by its own rule the predictor is dead. A reader of the results doc sees only confirmations.

**2. "The claim-bound arms reproduce on the clean channel exactly" is false for RTD-9, and a later prediction was built on it.**
- (a) The doc says the clean re-score changed nothing for RTD-9, RTD-10 and RTD-3; RTD-9's arm 1b moved.
- (b) `e2s-score-rtd9.txt` gives arm 1b **4/9** on the clean channel; `fsc-rtd9.txt` gives **3/9** on the contaminated one. The doc's own table (line 410) prints "4/9 [3/9]" next to the sentence "reproduce … exactly" (line 423).
- (c) The API re-score prediction (prereg line 387) cites this as its reason: "RTD-9 and RTD-10 reproduce, since their fork-route rows reproduced exactly". The "denominator" claim that contamination changed no comparison is overstated.

**3. The "12 of 12 runs" behind the clean-2 ruling is contradicted by the logs.**
- (a) clean-2 did **not** fire `member_vs_population` on 12 of 12 runs "of both models on both channels".
- (b) The logs show:
  - `p1s-gate2.txt` (Haiku, dirty channel): 3/3;
  - `p1s-H0clean-gate.txt` (Haiku, clean channel): **1/3** — `[member, question_asked]`, `[question_asked]`, `[d_history]`;
  - `p1s-S0-gate.txt` (Sonnet, clean channel): 3/3.
  - No Sonnet run on the dirty channel exists. The real figure is 7 of 9.
  - The claim appears at `rule-tell-scoring-2026-09-23.md:340` and again in `phase1-local-classifier-preregistration.md:176`.
- (c) This was the stated basis for the operator's ruling and for form 2b's narrowed spec. For Haiku on the clean channel, clean-2 failed mostly through *other* rules, so "the fixture and the spec disagree" explains only part of the failure.

**4. The L0 "ranking carries signal" reading, which is handoff item 3's lead for Stage 3, overstates the data.**
- (a) The claim is that gold ranks 1st or 2nd on 4 of 5 violation texts **and** scores above that rule's clean maximum. On the committed probabilities, both conditions together hold for 2 of 5 texts (semicolon, member).
- (b) I rebuilt `l0-gate.jsonl` by row order; the fired sets match `l0-gate.txt` exactly.
  - `cannot_happen` scores 0.600599 on the cannot text and exactly 0.600599 on clean-1, so it is not "above" (the doc's own table shows +0.00).
  - `d_sessionid` is exactly 0.5, tied with 3 other rules for ranks 2–5.
  - contradiction is tied for ranks 12–14.
  - The 220 values take only **41 distinct levels**, and 8 rows are exactly 0.5.
  - The sessionid gate "pass" rests on p == 0.5 meeting the `>=` threshold (`phase1-local-l0.py:56`).
- (c) Per-rule calibration is chosen as the Stage-3 lead on a rank signal that ties and quantisation largely erase.

**5. The Score B sensitivity analysis is one-sided.**
- (a) The doc stresses that RTD-10's pass depends on excluding unobservable forks. RTD-9's fail depends on the same exclusion, and the doc doesn't say so.
- (b) `fork-e2s-dp1.jsonl`:
  - Run 2 (no tool call) opens by **retracting** the RTD-10 claim ("Am spus că … «conflația nu poate să apară» … o interzice explicit").
  - Run 5 **scopes** "a scris o dată" to its selector.
  - Both are excluded as unobservable.
  - Counting RTD-9's 3 unobservable forks as compliant gives 3/10, and 0.7 − 0.3 = 0.40 meets the rule (RTD-9 has only that condition).
- (c) "Three of four rules fail" mis-describes RTD-9; its result is as fragile as RTD-10's. This is the same recording-filter effect the doc diagnosed at RTD-3. The overall verdict (S0 does not ship) still stands on RTD-8 and RTD-3.

**6. RTD-8's observable differs from the other checkers', but the doc describes one convention for all of them.**
- (a) The doc says forks with a different first action are excluded. For RTD-8/`rtd8c`, such forks are scored if their text mentions the table.
- (b) The code (`phase2-score-dp1.py`):
  - `rtd8`/`rtd8c`: `"pika_observations" in r["text"]` (line 61);
  - `rtd9`/`rtd10`/`rtd3`: `"mcp__codescout__doc" in r["tools"]`.
  - Forks scored for RTD-8 with no doc write: fork-dp1 arm 1b run 2 (`run_command`), arm s1b runs 6 and 7 (no tool), e2s runs 2 and 5.
  - The same rows count as unobservable for RTD-9/RTD-10. The false convention statement is at doc lines 181 and 452.
- (c) The RTD-8 fork rates mix pre-decision replies into the denominator (for example, arm 1b 2/10 would be at most 2/9). No verdict flips, but the RTD-8 and RTD-9/RTD-10 cells on the same forks don't share a denominator.

**7. The next steps in the phase-1 local pre-registration are stale.**
- (a) Stage 2's held-out set and Stage 4's gate and Score B still describe the pre-amendment state.
- (b) `phase1-local-classifier-preregistration.md`:
  - line 71 holds out "the 8 gate texts": clean-5 and member are missing, so the 8-token leakage filter won't exclude them;
  - line 95: the Stage-4 gate is "the 8 texts";
  - line 99: Score B uses `rtd8`, which failed its clean-channel gate (`e2s-score-rtd8.txt`), rather than `rtd8c`;
  - line 27: the comparison rows are the contaminated ones ("arm 1b for RTD-8 (0/10)"), not `rtd8c`'s 2/10.
- (c) Following the plan as written risks leakage into training and makes Score B run a checker known to be invalid.

**8. "The judge channel moves rates on identical replays" is confounded for RTD-10, and has no committed evidence.**
- (a) RTD-10's 5/10 → 9/10 shift also spans a checker rewording, not only the channel.
- (b) Prereg lines 221–222: RTD-10 was reworded a second time *on* the subscription channel. `score-rtd910.txt` contains only usage-cap tracebacks. No committed file holds the API-judge 4/10 (RTD-9) or 5/10 (RTD-10).
- (c) The claim that the channel shift is "as large as some effects under test" rests on an uncontrolled comparison.

**9. The default channel of `phase2-score-dp1.py` is the contaminated one.**
- (a) It defaults to the contaminated channel and has no guard against it.
- (b) Line 213 defaults `JUDGE_CONFIG_DIR` to `~/.claude-kat`. `phase1-span-selector.py:451-462` refuses a dirty config; this script doesn't.
- (c) Any re-score run without the environment variable silently produces contaminated-channel rates. This is documented, but not enforced.

**10. The README's "this directory is the evidence" is incomplete.**
- (a) Several published results have no committed rows:
  - the Jev `noul` mutation-gate table;
  - the reasoned-form 5/5 gate at n = 10;
  - the native form's 10/10 NO;
  - Jev's end-to-end `none` on 10/10 (p 0.23–0.46);
  - the phase-1A gates (6/6 and 4/6).
- (b) `grep -il "noul\|mutation gate"` over the data directory hits only L0 files and the research reports. `l0-gate.jsonl` rows carry no text id, so they can be rebuilt only by row order. `l0-gate.txt` labels JevK5 "subscription judge", a string inherited from the shared gate code.
- (c) These results cannot be checked from the repository.

**11. Several small miscounts in the gate tables.**
- (a) Form 1:
  - clean-1 fired 4–5 rules, not 3–5;
  - cannot drew +11–13 extra rules, not +12;
  - contradiction drew +11, not +10.
- (b) S0's contradiction row reads "+0–4"; the log shows +1–4. The prereg's "violation texts fired 4–13 unrelated rules" has a minimum of 3 (sessionid, run 3). Source: `p1s-gate.txt`, `p1s-S0-gate.txt`.
- (c) Cosmetic; no verdict depends on these.

**12. The DP1 limits paragraph cherry-picks one Jev run.**
- (a) It says Jev "ranked the correct rule first (contradiction, 0.40)" on this case.
- (b) In `phase1-jev.jsonl`, RTD-8's positive picks are `contradiction` 0.40, **`none`** 0.34, then `contradiction` 0.35. The corrected (negative) side picks `contradiction` at 0.86–0.87 on all 3 runs.
- (c) Reading this as evidence that phase 1 could produce the binding overstates it: on this pair Jev picks the same rule for the violation and its fix.

**13. Stale text in the scoring doc.**
- (a) Line 137 still places the RTD-3 decision point at "record 1290" and says it "has not been replayed".
- (b) Prereg line 253 corrects this to record 1498, and the doc's own line 200 says 1498.
- (c) Low impact; the two parts of the doc contradict each other.

**14. The `rtd3r` registration contradicts itself.**
- (a) It says "a bare `grep` stays unobservable", but also counts tool input as text, as the code does.
- (b) `phase2-score-dp1.py:117` treats any non-empty text as observable, and fork rows put tool input in `text`. Prereg line 280 says both things.
- (c) Harmless on this data: no row opens with a tool call and no prose; I checked all 100 RTD-3 rows. A future run could still diverge from its registration.

## Checks that came out clean
- **Phase 0:**
  - positives: CTL10-8 1.0, CTL8-6 0.9, CTL3-6 0.8, CTL9-4 0.0;
  - the diagonal near-miss and full-shape means and fire counts, and the off-diagonal rates, all reproduce from `scored-reasoned-n10.jsonl`;
  - the retraction's claims: 44/44 one-directional disagreements, 22 fractional YES verdicts, 72% of structured-path rows with no reasoning, and native near-miss 0/8.
- **Phase 1A:** Jev 12/51, 27/51, 30/63; Haiku 12/85, 30/85, 1/105; mean P(none) 0.27/0.24; distributions differ on 42/42 states, top pick stable on 33/42.
- **Score A:**
  - form 2b and form 3 tables, negative fire sets and newly fired rules;
  - the silence/wrong-rule tables (13/1/3 and 10/4/3);
  - family recall 3/17 and 5/17;
  - Wilson interval [0.06, 0.41];
  - 924 rows with 22 per text.
- **Phase 2 rates:** every arm cell in the API, fork, `rtd8c`, e2s and `rtd3r` tables matches its score file, and every ship-rule difference is computed correctly.
- **RTD-3:**
  - grep-first counts (3/6/3, and 6 in the old run);
  - 7 no-tool 3-1b forks, and I read all 7: each retracts the claim;
  - the alarm about observable/registration divergence, correctly withdrawn;
  - the arithmetic bound.
- **e2s data:** injections (2 of 10 DP1 drafts had no injection; 6 of 10 RTD-3 drafts; `monotone_absence` in runs 0 and 7) and the fork tool counts.
- **Stripped-arm file:** 17 of 30 rows are errors.

`★ Insight ─────────────────────────────────────`
- The same pattern shows up in several places: a filter on the observable hides successes. RTD-3 found it and fixed it. Score B DP1 has it again (a retraction and a scoped count excluded as "unobservable"), and the doc reports only the direction that hurts.
- Re-deriving L0's ranks from raw probabilities showed quantised outputs: 41 levels, several exactly 0.5. A rank-based signal is only as good as the resolution under it, and a `>=` threshold at an exact tie decides a gate pass.
- Most of the numbers reproduce exactly. The defects sit in prose summaries next to correct tables: "reproduce exactly" printed beside 4/9 [3/9], and "12/12" when the logs give 7/9. Recomputing is what catches these, not re-reading.
`─────────────────────────────────────────────────`