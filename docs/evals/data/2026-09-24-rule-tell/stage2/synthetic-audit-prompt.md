# Synthetic pair audit prompt — Stage 2 (fixed; registered in the phase-1 local-classifier pre-registration, synthetic amendment, corrections 6 and 8)

You are auditing generated data for a classifier. Each item is one generated PAIR for ONE rule.
You are given the rule's LAW and its usual violation shape (the spec), and two versions of a
paragraph:

- `original` — the paragraph as generated, with the target sentence marked ⟦like this⟧.
- `substituted` — the same paragraph with the target sentence replaced by its fix, the fix
  marked ⟦like this⟧.

Judge by the rule's LAW. The spec describes the usual shape and is guidance; where they
disagree, the law wins. Judge only from what is shown.

## Answer three questions per item

- `a` — Does the marked sentence in `original` break the rule? `yes` or `no`.
- `b` — In `substituted`, read AS A WHOLE, is the rule no longer broken by the marked
  sentence? `yes` if the fix removed the breach in context, `no` if the breach remains (in the
  sentence, or because the rest of the paragraph still makes it one).
- `c` — Does any OTHER sentence of `original` break this same rule? `yes` or `no`.

A pair passes only with `a = yes`, `b = yes`, `c = no`. Do not grade style or realism.

## Output

One JSON object per item, one per line, nothing else:

    {"pair_id": "<pair id>", "a": "yes|no", "b": "yes|no", "c": "yes|no", "note": "<at most 25 words>"}

Answer every item you are given, with no item skipped or duplicated. An answer that is not
one of `yes`/`no`, or a missing item, is retried once and then counted as a disagreement.
