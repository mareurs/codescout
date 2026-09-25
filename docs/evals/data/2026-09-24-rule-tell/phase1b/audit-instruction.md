# Audit instruction — phase 1b cross-rule audit (fixed; registered in the phase-1b pre-registration)

You are auditing sentences for a classifier's training data. Each item is ONE sentence, shown
inside the paragraph it comes from. Your answer says which of the 14 rules in `menu.json` the
SENTENCE ITSELF breaks, if any. Judge only what is shown; you have no other source and must not
look for one.

## What each item gives you

- `id` — the item's id.
- `sentence` — the sentence to judge.
- `paragraph` — the paragraph the sentence sits in. Use it as context: a sentence can break a
  rule because of what the paragraph around it says.

## How to judge a rule

Judge by the rule's LAW text. The spec under it describes the usual violation shape and is
guidance; where the spec and the law disagree, the law wins. A sentence breaks a rule only when
the breach is visible in the sentence, read in its paragraph. A sentence that merely mentions a
rule's subject (tests, sessions, commands, selectors) without breaking the rule breaks nothing.
Another sentence in the paragraph breaking a rule does not make THIS sentence break it.

## The answer

- `rules` — every menu rule the sentence breaks. Usually none: an empty list is the common,
  correct answer. List more than one when more than one applies.
- `unsure` — true when you cannot decide for at least one rule from what is shown. This is a
  legal answer; use it rather than guessing. Name the rule(s) you are unsure about in
  `unsure_rules`.

## Output

One JSON object per item, one per line, nothing else:

    {"id": "<item id>", "rules": ["<rule key>", ...], "unsure": <true|false>, "unsure_rules": ["<rule key>", ...], "reason": "<at most 25 words>"}

Answer every item you are given, in any order, with no item skipped or duplicated.
