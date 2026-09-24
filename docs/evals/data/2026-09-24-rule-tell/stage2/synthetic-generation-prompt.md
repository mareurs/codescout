# Synthetic pair generation prompt — Stage 2 (fixed; registered in the phase-1 local-classifier pre-registration)

You are writing training and test data for a classifier that detects one kind of reasoning
error in technical writing. You will be given ONE rule (its law and its usual violation shape)
and SEED paragraphs taken from a software project's documentation. For each seed, write one
pair.

## A pair

- `paragraph` — a NEW paragraph of 60 to 200 words on the seed's topic, in the register of
  engineering notes: bug reports, design notes, eval write-ups, handoffs. It contains exactly
  ONE sentence that breaks the given rule. That breach must be visible from the paragraph
  itself, without outside knowledge.
- `violating_sentence` — that sentence, copied verbatim from `paragraph`.
- `fixed_sentence` — the same sentence with the smallest edit that stops it breaking the rule.
  Prefer REPLACING words over ADDING them: name the unit instead of the bare number, state
  what was actually checked instead of what was concluded, narrow the scope word. The fix
  should be about as long as the original, and must not work by adding a hedge (`may`,
  `likely`, `appears`, `probably`) unless the law itself is about overclaiming. Replacing
  `violating_sentence` with `fixed_sentence` must leave a paragraph that reads naturally, and
  **the replaced paragraph as a whole must no longer break the rule**: a fix that removes
  the breach from the sentence while the paragraph still carries it is not a fix.
- `why` — at most 25 words: what in the violating sentence breaks the law, and what the fix
  changes.

## Constraints

- Judge by the rule's LAW. The shape describes the usual form; vary the surface form across
  pairs, and do not reuse the shape's wording. A classifier trained on one phrasing learns the
  phrasing, not the rule.
- The other sentences in the paragraph should break none of the rules you know of. Make them
  well-founded by showing their basis, not by hedging: at least one of them should be a
  CONFIDENT, unhedged claim whose basis the paragraph shows. The violating sentence need not
  be the most confident one; where the law allows, it may itself be hedged.
- Put the violating sentence at different positions across pairs: first, middle, last.
- Do not copy 8 or more consecutive words from a seed. Take its topic, names and vocabulary,
  not its sentences.
- Never name the rule, and never use the words `violation`, `violates`, `rule`, `incorrect`,
  `correction`, `corrected`, `wrong` or `fix` in `paragraph` or in either sentence.
- The fixed sentence must not simply delete the claim. It must still say something the
  paragraph needs.

## Output

One JSON object per seed, one per line, nothing else:

    {"seed_id": <seed id>, "rule": "<rule key>", "paragraph": "...", "violating_sentence": "...", "fixed_sentence": "...", "why": "..."}
