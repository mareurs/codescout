I found seven defects, and two of them change what the documents conclude. The worst is that Stage 2's held-out filter lets through a candidate from the same correction commit as two Score A cases. Second, the form 4q result rests partly on a surface trigger: the widened spec fires on `file:line` citations whether or not they break the rule.

## Findings

**1. The Stage 2 held-out filter keeps a candidate from the same commit and document as two Score A cases.**
- **(a) Defect:** the miner's leakage check only looks for matching 8-word runs of text. A candidate from the held-out incident, worded differently, passes, even though Amendment 3 requires one fold per incident.
- **(b) Evidence:**
  - `stage2/mined-candidates.jsonl:939` is sha `e365a6b3`, path `docs/adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md`, incident `…@ddfce06545`.
  - Its positive sentence: "…the existing split between 15 params-backed and 16 prose-backed trackers where only the former have lost data".
  - `docs/evals/rule-tell-detection.md:779` and `:805` give `e365a6b3` as the source commit of two corpus cases. `:859` gives `ddfce065` as RTD-16's source.
  - The sentence restates the very claim RTD-16 and RTD-21 are built on ("15 of 31 augmented trackers… remaining 16 are prose"; "a clean params-versus-prose split that the data does not support").
  - I listed every kept candidate against the 13 source SHAs and the 3 source documents the corpus names; this is the only hit.
- **(c) Impact:** labelled `count_unit` or `closed_population`, this would train on a held-out answer. The prereg's claim at `:362` that the build "applies… incident grouping… against every held-out source" is false.

**2. A candidate from this campaign's own documents survived the "campaign documents" filter.**
- **(a) Defect:** `HELD_OUT_DOC_RE` (`mine_pairs.py:54-56`) matches `rule-tell`, `rule-injection`, `phase1-local-classifier` and `codex-rule-tell-review` in the path. It does not match `docs/trackers/prompt-hamsa-audit-log.md`, where this campaign logged its results.
- **(b) Evidence:** `mined-candidates.jsonl:944` is sha `d8e6f4d5`, dated 2026-09-24. Its positive sentence: "**"The claim-bound reminder stops RTD-8" is withdrawn** pending `rtd8c`". Its paragraph quotes the 2,778-vs-249-token judge-contamination measurement.
- **(c) Impact:** contradicts the prereg's `:362` claim of "dropping anything from this campaign's own documents". Training could see text about RTD-8.

**3. The "no rule reaches 50" argument ignores the rule with the most hints.**
- **(a) Defect:** prereg `:371` says "`count_unit` is nearest at 50 raw hints, about 15 after the sample's precision". That is false by the committed summary.
- **(b) Evidence:**
  - `stage2/summary.txt` gives `closed_population` 141 single hints and 169 counting every hint, against `count_unit`'s 48 and 50.
  - The doc's own method, 169 × 0.30–0.40, gives about 51–68, which is at or above 50.
  - The method itself doesn't hold up either. It multiplies keyword-hint counts (which `:369` says are not labels) by a genuine-pair rate taken from 10 rows, with no interval. 615 of the 946 candidates have no hint at all.
- **(c) Impact:** the conclusion that the route "cannot run on mined pairs alone" may be right, but no valid derivation of it is published. It is one of the stated inputs to the (a)/(b) labelling decision.

**4. RTD-12's hit is not on the audit's sentence.**
- **(a) Defect:** scoring doc `:680` says the two hits are "each quoted on the audit's own sentence". For RTD-12 that is false.
- **(b) Evidence:**
  - The audit's main quote for RTD-12 is "So **id allocation already survives a catalog loss**…" (`partial-audit-verdicts.json`, RTD-12).
  - S0 quoted a different sentence, the one holding the caveat the audit recorded as `quote2`: "The allocator's durability was verified at `append_entry.rs:294-324`; … **not exercised end-to-end…**" (`p1s-S0f4q-corpus.jsonl`, fresh row).
- **(c) Impact:** the evidence that the widened spec finds the audited violation holds for RTD-1 only. See finding 5.

**5. The widened `question_asked` fires on `file:line` citations as such, and the per-text metric hides it.**
- **(a) Defect:** half of the new fires are wrong. The reading "the silence was spec coverage… with no new false-positive text" (`:684`, repeated at `:703` and in the operator summary) leaves that out.
- **(b) Evidence:**
  - Form 4q made 4 fresh `question_asked` fires (`form4q-readout.txt`). Only RTD-1 and RTD-12 are gold.
  - The other two are false: RTD-8's positive ("verified at `src/usage/db.rs:323-339`") and RTD-13's clean negative ("refused on a prose ledger (`append_entry.rs:194-201`)").
  - 3 of the 4 fires are on sentences quoting a `file:line`, including RTD-12's.
  - RTD-13's negative gained a false fire, but it already fired `run_tool`, so "negatives with any fire" stays 5/21. The "no" bucket's fires per negative text doubles, 0.25 → 0.50 (`p1s-S0f4q-corpus.txt`), and the scoring doc's table leaves that column out.
  - The operator summary's "No clean text newly fires" is true only when counting texts.
- **(c) Impact:** "spec coverage explains the silence" is over-read. The hits may reflect a surface trigger (a `file:line` citation) that the tailored wording names, not the violation itself.

**6. The form 4 gate write-up credits the widening for clean-4, but clean-4 was already borderline under form 3.**
- **(a) Defect:** `:659` says widening "made the judge apply" `scope_instant` to clean-4.
- **(b) Evidence:** `form3-gate.txt` shows clean-4 with `['count_unit', 'scope_instant']` in 1 of 3 runs under the unwidened form-3 specs. It passed at exactly the 2/3 threshold.
- **(c) Impact:** half of P1's failure (clean-4) is not cleanly attributable to the widening. Only clean-3 (0/3 fires under form 3) is. Separately, `:658` generalises one failed wording on two clean texts into "the 'derive it' half… is not expressible as a sentence-level shape". The doc's own next step (T) is the test of that, so it should not be stated as a finding.

**7. Reproducibility defects in the committed data scripts.**
- **(a) Defect:** README `:78` says "the script regenerates" the 97 MB git log. `mine_pairs.py` does not; it only reads `HERE/gitlog.patch`.
- **(b) Evidence:**
  - The docstring's command runs `git log experiments`, a branch that is rebased after every ship.
  - `mine_pairs.py:38` and `form4q-readout.py:4` hard-code `ROOT=/home/marius/work/claude/codescout`. They import the selector, and read the held-out texts and the form-3 rows, from whatever state the main checkout is in, not from the committed tree.
  - `--carry` records no model or form on its rows. Carrying from a different judge would be accepted silently; the 4q run's source was consistent (Sonnet 5, form 3, from the log header).
- **(c) Impact:** the candidates can't be re-derived once the author's session-local scratchpad snapshot is gone.
- **Minor:** the operator summary says the 92 dropped rows all "overlapped held-out texts or came from this campaign's own documents". 2 of them were duplicates; the prereg (`:364`) states this correctly.

## Checks that came out clean

- **Carry merge:** all 882 carried rows in `p1s-S0f4q-corpus.jsonl` are byte-identical to the form-3 rows. 42 rows are fresh, all `question_asked`. The file totals 924 rows over 42 texts.
- **Recalculated from rows:**
  - recall 3/17 → 5/17;
  - "yes" bucket gold-only 2/8 → 1/8;
  - negatives with any fire 5/21 → 5/21;
  - gate 6/8 (form 4) and 6/6 (form 4q);
  - P2's list of nine positives whose gold includes a widened rule matches `GOLD`.
- **Miner reproduces exactly:** the committed `mine_pairs.py`, pointed at this tree and run on the author's git-log snapshot in `/tmp/rv-s2-claude-primed-0/`, gives the same `summary.txt` and a byte-identical `mined-candidates.jsonl`. Drop counts add up: 2 + 20 + 70 = 92.
- **Registration timing:** file times fit each registration commit landing before its run (form 4 gate finished 19:34:14 after `15c37237` at 19:32:20; form 4q finished 19:37:52 and 19:40:54 after `6aa61dff` at 19:36:27).
- **Tests:** `python3 -m unittest` on both test files, 17 passed.
- **Archive commit `938e6d0c`:** no citation to either archived bug's old path remains at HEAD.
- **Random sample:** re-reading the 10-row sample, I agree with "3 genuine, 1 weak". An interval is owed for n=10: Wilson 95% for 3/10 is about 11–60%.

`★ Insight ─────────────────────────────────────`
- A text-overlap filter (8-word runs) catches copies, not the same incident reworded. Finding 1 is exactly the case Amendment 3's incident rule was written for, and the miner never checks against the held-out cases' source commits, although the corpus lists them.
- Findings 5 and 6 are the repo's own "population metric hides a member" law: the "any fire per text" metric absorbs a new false fire when the same text already fired another rule.
`─────────────────────────────────────────────────`