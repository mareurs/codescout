---
status: open
opened: 2026-09-13
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/assertion-that-cannot-fail
kind: bug
---

# Two of three cross-language parity arms compare empty to empty

## Summary

`the_hook_script_agrees_on_the_cluster_parsers` (`tests/issue_clusters.rs`) compares three JSON
keys from `scripts/pre-commit-ledger-counts.py --source=worktree --json` against their Rust twins.
On today's corpus two of those three are **empty on both sides**, so they assert that nothing
equals nothing.

That is not an accident and not drift: `declared` and `claimed` are empty *because the ledger is
correct*. `no_index_row_stores_a_count` requires `declared` to be empty and
`no_class_field_states_a_bare_n` requires `claimed` to be empty. The parity test's population is
exactly what two other rules exist to keep at zero.

## Symptom (Effect)

The test passes, and would keep passing if the Rust parser or the Python parser stopped matching
altogether — a divergence in which *both* sides return nothing is invisible to an equality check.
Only a divergence in which one side starts producing output can be caught.

## Reproduction

Measured 2026-09-13 against `4f268eb1`:

```
$ python3 scripts/pre-commit-ledger-counts.py --source=worktree --json
{"actual": { ...24 populated entries... }, "claimed": [], "declared": {}}
```

`the_hook_script_agrees_on_the_cluster_parsers` compares `claimed` (empty), `declared` (empty) and
`actual` (populated). One of its three arms carries the whole test.

## Root cause

The `n` column was removed from the roster's Index table on 2026-09-02, and bare `n=` values were
removed from the class fields around the same time. `parse_index_counts` and `parse_bare_n_claims`
both survived that change and both correctly now find nothing. The parity test was written when
they found something, and nothing re-examined it when its population went to zero.

This is the population-scope law in `CLAUDE.md` § *Testing Discipline*: an assertion computed over
an empty population is vacuous for every member, and reads as coverage.

## Why this is medium and not high

The two parsers are **not** unguarded. `the_ledger_parsers_agree_on_a_fixture` drives both across
the language boundary over a synthetic ledger with known non-empty answers, and
`the_index_row_parser_discriminates` and `the_bare_n_claim_parser_discriminates` prove each is not
vacuous. So the coverage exists; what is misleading is the *live-corpus* test, which reads as a
third, independent confirmation and is not one.

The cost is therefore false confidence rather than a hole — but false confidence in a test named
`..._agrees_on_the_cluster_parsers` is the shape that stops the next person looking, which
`CLAUDE.md` names as the more expensive direction.

## Suggested fix

Not started. In rough order of value:

1. **Annotate the arms as inert**, on the assertion line, saying which sibling actually holds each
   parser. `CLAUDE.md` asks for exactly this — *"annotate an inert fixture as inert, so nobody
   credits it with coverage it does not provide."* Cheapest, and it fixes the misleading half.
2. **Assert the emptiness deliberately** rather than incidentally: `assert!(declared.is_empty())`
   with a message saying the roster stores no counts, which turns two vacuous comparisons into one
   real (if weak) claim, and reds if the column ever returns.
3. Drop the two arms and let the fixture test own them. Loses nothing measurable; loses the
   ability to notice a Python-only regression against real data, which is what the test was for.

Option 1 and 2 compose and are probably both right.

## Tests added

None yet.

## Resume

Nothing in flight. Noticed while adding `no_index_row_stores_a_mechanism`, whose own Python-driving
discriminator was written with an explicit non-empty assertion precisely to avoid this shape:

```rust
assert!(!theirs.extra.is_empty(), "the Python side returned nothing on a fixture with two
        planted findings — the check is present but no longer matching, which on the live
        corpus is indistinguishable from a clean ledger");
```

That assertion is the pattern the arms here lack.

## References

- `tests/issue_clusters.rs` — `the_hook_script_agrees_on_the_cluster_parsers`, and its own doc
  comment, which already concedes the live-corpus substrate is not adversarial. It does not say
  the population is empty; the concession and the measurement are different claims.
- `docs/conventions/what-green-is-evidence-for.md` — the population-vs-member law.
