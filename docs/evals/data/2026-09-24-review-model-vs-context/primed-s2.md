All three are done and committed; nothing is pushed. The selector work partly worked: widening the specs made S0 fire on the texts it had missed, but only for one of the three rules. Stage 2 is blocked on a labelling decision that's yours to make.

**1. Scorer bugs archived** (`938e6d0c`). Both bug files are moved to `docs/issues/archive/` with their history kept. Every file that cited the old paths is updated in the same commit, and both test files still pass.

**2. Next selector registration: two diagnostic runs.** I rewrote the specs for three rules to name the violation shapes the audit found. That wording was written after reading those cases, so these runs show whether S0 *can* fire, not that the wording would work on new texts.

- **Form 4, all three rules widened** (`15c37237`, `f828134a`):
  - The gate failed at 6 of 8, so Score A was not run, as registered.
  - The widened `count_unit` flagged ordinary counts in clean text ("the three tests", "the 12 rows"), and `scope_instant` flagged one of them too.
  - The underlying rule for counts, "derive it, don't cite it", can't be written as a sentence pattern without catching harmless counts.
- **Form 4q, only `question_asked` widened** (`6aa61dff`, `e60426a9`). I chose this arm after seeing form 4's gate, and the registration says so.
  - The gate passed 6 of 6.
  - S0 now finds the violation in RTD-1 and RTD-12, the two cases the audit had found fully visible for this rule.
  - No clean text newly fires. Recall goes from 3 of 17 to 5 of 17, still well below the 0.5 target.
  - **One cost I hadn't predicted:** `question_asked` also fires on RTD-8, next to that text's correct rule.
- **Conclusion:** for `question_asked`, S0 was silent because of the spec's wording. For the other two rules, naming the shape costs too much precision. Whether any of this wording works on unseen texts can only be tested on the future test set T, since I've read all of these ones.

**3. Stage 2 started** (`f828134a`). A miner that makes no model calls pulled 946 candidate correction pairs from the repo's history. It dropped 92 that overlapped held-out texts or came from this campaign's own documents. The candidates are not usable as training data yet:
- **Quality:** in a 10-pair sample, only about 30–40% are real before-and-after corrections. The rest pair two unrelated sentences, or are fragments.
- **Volume:** no rule gets near the 50 examples needed to train it from these pairs alone.
- **Labels:** a correction shows which sentence changed, not which of the 22 rules it broke, and the plan doesn't say who assigns that.

**Decision needed from you before Stage 2 can go further:** who labels the mined pairs? Either:
- **(a) A named labeller** labels every pair against the specs, with a second labeller checking a sample and agreement published, or
- **(b) Synthetic pairs first.** Generated examples carry their rule from the start, and the mined pairs become a smaller, checked source.

Both options are written into the pre-registration's Stage 2 status section.

The miner, its candidates and every run's rows and logs are in `docs/evals/data/2026-09-24-rule-tell/`, and codescout memory has the current state.
