**Review: the rule-tell campaign since `9c0d2505` (S0 form 4, form 4q, Stage 2 mined pairs)**

The form 4 and 4q numbers are correct and reproduce from the committed rows. The defects are in the Stage 2 note, which draws conclusions its own data doesn't support, and in some interpretation of form 4q. Scratch files are under `/tmp/rv-s2-claude-cold-1/`; nothing in the checkout was changed.

## Findings (most severe first)

1. **The Stage 2 stopping-rule reading misnames the rule nearest the ≥ 50 bar.**
   - (a) `phase1-local-classifier-preregistration.md:371` says "`count_unit` is nearest at 50 raw hints, about 15 after the sample's precision". But `closed_population` has more than three times as many hints.
   - (b) `stage2/summary.txt` lists `closed_population` at 169 (any hint) and 141 (single hint), against `count_unit` at 50 and 48. The note's own method (raw hints × 30–40%) gives `closed_population` about 51–68, which crosses 50.
   - (c) "No rule reaches 50 from mined pairs alone" only holds if `closed_population` is quietly left out. That exclusion is argued in a different bullet (`:369`) and never applied here. The next-step reasoning ("the route cannot run on mined pairs alone, it needs synthetic pairs") therefore rests on an unstated exclusion. Also, the 30–40% is a pair-genuineness rate from 10 rows; it measures whether a pair is real, not whether the rule hint is right, so neither estimate is actually derived.

2. **The recommended "stricter subset" (marker in the corrected sentence, 230 rows) is not stricter for about 43% of its rows.**
   - (a) In 101 of the 230 `marker_source="twin"` rows, a correction marker also appears in the positive. In 98 of them it is the same word, so it was already there before the edit and says nothing about the edit being a correction.
   - (b) I re-ran `MARKER_RE` over `stage2/mined-candidates.jsonl` and got 101 and 98. Examples:
     - `3b68521a`: "originally marked **not established**" appears on both sides.
     - `682cb4f8`: "narrower" matches inside the tag slug `guard-narrower-than-its-name`, and the edit is only a count bump, n=11 → n=12.
   - (c) The prereg (`:367`) steers the next build toward this subset. Filtering on it would keep many rewordings and bookkeeping edits as "corrections".

3. **The miner lets a Score A incident into the training candidates.**
   - (a) `HELD_OUT_DOC_RE` (`stage2/mine_pairs.py:57-59`) drops only the campaign's own documents, not the source documents of the 21 Score A cases. The 8-token shingle filter misses paraphrases.
   - (b) Candidate `e365a6b3` in `docs/adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md` reads "…the existing split between **15 params-backed and 16 prose-** backed trackers". That is the same claim as RTD-16's gold `count_unit` positive ("15 of 31 augmented trackers… the remaining 16 are prose ledgers"), from the same ADR, and `e365a6b3` is also RTD-12/13's source commit. I found it by listing the RTD source SHAs in `rule-tell-detection.md` and intersecting them with the candidates' `sha`.
   - (c) If this row is labelled `count_unit`, a trained arm's Score A result on RTD-16 is contaminated. Amendment 3 folds "the other paragraphs of the same source document" by incident, but nothing does that for Score A's source documents. Only one row is affected today; the mechanism, though, is a missing filter category.

4. **Form 4q's widened `question_asked` fires on the surface shape "verified at `file:line`", and the gate cannot see that.**
   - (a) Three of the four `question_asked` fires are sentences of the form "verified at `x.rs:N-M`". Two of those three are not gold: RTD-8's positive and RTD-13's negative. The widened spec's own wording ("a file:line reading cited as proof a path behaves some way") makes those fires correct *by spec*.
   - (b) The four fires, with their quotes, are in `form4q-readout.txt`. None of the gate's clean texts (`EXTRA_GATE`, `phase1-span-selector.py:222-235`) contains a `file:line` citation, so the gate passing 6/6 says nothing about this direction.
   - (c) The published reading "for `question_asked`, the silence was spec coverage… no new false-positive text" (`rule-tell-scoring-2026-09-23.md`, form 4q section) rests on n=2 hits. One of them (RTD-12) is the same surface pattern that fired on a clean negative. The "no new FP text" result holds only because RTD-13's negative already fired `run_tool`; any-fire per text is blind to extra fires on a text that already fired. Calling RTD-8's fire a "wrong rule" also contradicts the spec's own wording. On T, this clause can be expected to fire on ordinary well-cited prose.

5. **The form 4 write-up misattributes `scope_instant`'s clean-4 fire.**
   - (a) The scoring doc says the clause "a count of changing state with no time" "was in form 3's spec already; widening the rest made the judge apply it". But form 4 also changed that clause.
   - (b) In form 3 (`phase1-span-selector.py:71`) it reads "a count of changing state (sessions, open items) with no time", and the NO clause is "not the result of a search". In `SPECS_F4` the examples in parentheses are gone and the NO clause becomes "reports no search or measurement at all".
   - (c) The causal claim can't be separated from the clause's own widening, so it should not be carried forward as a finding about the judge.

6. **About 24% of mined candidates have an incident key that doesn't identify the incident.**
   - (a) For 223 of 946 rows, `incident` ends in the correction commit's own SHA. That is the fallback in `first_seen.get(h, r["sha"])` (`mine_pairs.py`, `main`): the positive sentence was never matched to the commit that added it, usually because sentence splits differ across hunks.
   - (b) There are also cross-file duplicates. One positive, "**Mechanism status:** none yet…", appears in four IC files, so it gets four incidents and four doc groups from one commit (`20083067`).
   - (c) Folding by incident (Amendment 3) will put near-copies into different folds. The note does carry sentence-splitting forward to the next build but doesn't report this rate.

7. **Three committed analysis scripts read a different working tree, not the commit they're in.**
   - (a) `stage2/mine_pairs.py:38`, `form4q-readout.py:4` and `partial-audit-check.py:4` hard-code `ROOT = /home/marius/work/claude/codescout`, which is the main checkout.
   - (b) The miner takes its held-out sources (selector `GATE`, controls, fork drafts) from that tree's current state, and the readout reads form 3's rows from there too.
   - (c) Re-running them from any other checkout, or after the main tree moves, silently produces results against different inputs. The README claims only the git-log extract as not kept; it doesn't mention this.

8. **Minor: form 4/4q wiring is untested, and rows don't record model or form.**
   - (a) `CarriedRuleSubset` only tests the carry merge. Nothing tests `SPECS = SPEC_FORMS[args.form]`, the `form.startswith("4")` gate extension, or the gate's n/a / `applicable` counting.
   - (b) Corpus rows record neither `model` nor `form`. So `p1s-S0f4q-corpus.jsonl` can't show that its 882 carried rows came from Sonnet form 3 and not, say, Haiku form 2b; only the `carried_from` path is recorded.
   - (c) This is low today: the observed fire changes show the new spec was in effect. But `--carry` would accept a corpus from a different judge without complaint.

## Checks that came out clean

- Form 4 gate log: 6/8, 0 errored, five n/a positives, clean-3 and clean-4 failing as the doc states.
- Form 4q gate: 6/6.
- `p1s-S0f4q-corpus.jsonl`:
  - 924 rows over 42 texts, exactly 22 rules per text; 882 carried and 42 fresh.
  - Every carried row matches form 3's row byte for byte once `carried_from` is removed.
  - The only fresh YES rows are the four `question_asked` fires.
- Recomputed Score A matches every cell of the doc's 3 → 4q table:
  - Recall is 3 → 5 of the 17 yes+partial positives.
  - Yes-bucket gold-only goes 2/8 → 1/8.
  - Negatives any-fire stays 5/21, per bucket 2/2/1.
  - Q1 = 2/3 (RTD-1, RTD-12), and RTD-11 is unchanged.
- Form 3 claims hold:
  - 10/17 positives are silent.
  - 4/17 fire a wrong rule (RTD-4, 9, 11, 20).
  - 0 of the 9 widened-rule positives fired.
- The gold lists in the P2 and Q1 registrations match the corpus.
- Stage 2 bookkeeping:
  - 1038 − 92 = 946.
  - Drops: 70 shingle, 20 held-out doc, 2 duplicate. The per-source drop counts overlap, which the prereg words correctly (the commit message's "92 after the held-out filter" includes the 2 duplicates).
  - 513 incidents, 250 doc groups.
  - 230 / 165 marker-source rows, 615 rows with no hint.
- My own read of the 10-row sample agrees with roughly 3–4 genuine pairs.
- No mined candidate shares an 8-token shingle with the three `GATE_F4` texts.
- The fork-draft files all carry `text`, so the fork held-out set is populated.
- `python3 -m unittest tests.test_phase1_span_selector_report tests.test_phase2_score_dp1`: 17 tests, OK.

`★ Insight ─────────────────────────────────────`
- Findings 2 and 4 are the same mistake at two levels. The miner treats "a marker word is present" as meaning "a correction happened", and the widened spec treats "a `file:line` citation is present" as meaning "evidence is answering the wrong question". Both keyword tests are checked only on inputs where the keyword and the meaning happen to line up (the gate's clean texts, the "twin" subset).
- "Negatives with any fire" per text cannot see a second fire on a text that already fired. That is exactly how RTD-13's new false positive stayed invisible to Q2.
- For Stage 2, the check that would catch leaks like finding 3 is an incident-level join (source SHA plus basename) against Score A's `source:` lines. Shingle overlap can't do it, because paraphrase defeats it.
`─────────────────────────────────────────────────`