# Labelling instruction — Stage 2 mined candidates (fixed; registered in the phase-1 local-classifier pre-registration)

You are labelling rows for a test set. Each row is a sentence that was later corrected in this
repository's documentation, with its context and its correction. Your label says which of the
22 rules below the ORIGINAL sentence broke, if any. Every row is judged on what is shown; you
have no other source and must not look for one.

## What each row gives you

- `positive` — the original sentence, before correction.
- `context_before` — the text around it before the correction (the sentence is inside it).
- `twin` — the corrected sentence that replaced it (absent for an appended-note row; then the
  row's `note` field holds the correction note that was appended after it).
- `subject` — the commit message subject of the correcting commit.

## The label — exactly one per row

- a rule key from the menu — the original sentence breaks that rule, and the correction
  repairs that breach. If two rules apply, give the main one as `label` and the other as
  `second_rule`.
- `not-a-violation` — a genuine correction, but of a factual or editorial error that breaks
  none of the 22 rules (a wrong name, a typo, an updated fact, a reworded sentence).
- `not-a-pair` — the positive and the twin are not one sentence and its correction: they are
  unrelated sentences, or one is a fragment.
- `unsure` — the row cannot be decided from what is shown. This is a legal answer; use it
  rather than guessing.

## How to judge a rule

Judge by the rule's LAW text. The spec under it describes the usual violation shape and is
guidance; where the spec and the law disagree, the law wins. A sentence that states a fact
without showing how it is known does not break a rule by that alone: the breach must be
visible in the sentence and its context, and the correction should be repairing it.

## Output

One JSON object per row, one per line, nothing else:

    {"id": <row id>, "label": "<rule key | not-a-violation | not-a-pair | unsure>", "second_rule": "<rule key or null>", "reason": "<at most 25 words>"}

Label every row you are given, in any order, with no row skipped or duplicated.
