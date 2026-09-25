# Synthetic pair audit prompt, relational v2 — for rules whose breach relates two statements (fixed; registered in the top-up amendment)

Use this version, not `synthetic-audit-prompt.md`, for `contradiction`. Its law is about two
statements that cannot both hold, so the sentence a marked target contradicts is PART of the
breach, not a second breach. Version 1's question (c) counted it as one.

You are auditing generated data for a classifier. Each item is one generated PAIR for ONE rule.
You are given the rule's LAW and its usual violation shape (the spec), and two versions of a
paragraph:

- `original` — the paragraph as generated, with the target sentence marked ⟦like this⟧.
- `substituted` — the same paragraph with the target sentence replaced by its fix, the fix
  marked ⟦like this⟧.

Judge by the rule's LAW. The spec is guidance; where they disagree, the law wins. Judge only
from what is shown.

## Answer three questions per item

- `a` — In `original`, does the marked sentence take part in a breach of the rule? For a
  contradiction: is there another statement in the paragraph that it cannot both hold with?
  `yes` or `no`.
- `b` — In `substituted`, read AS A WHOLE, is that breach gone? `yes` if replacing the marked
  sentence removed it in context, `no` if the same breach remains.
- `c` — Leaving aside the breach the marked sentence takes part in (and the statement it
  contradicts), is there a SEPARATE, independent breach of this rule anywhere in `original`?
  `yes` or `no`.

A pair passes only with `a = yes`, `b = yes`, `c = no`. Do not grade style or realism.

## Output

One JSON object per item, one per line, nothing else:

    {"pair_id": "<pair id>", "a": "yes|no", "b": "yes|no", "c": "yes|no", "note": "<at most 25 words>"}

Answer every item you are given, with no item skipped or duplicated. An answer that is not
one of `yes`/`no`, or a missing item, is retried once and then counted as a disagreement.
