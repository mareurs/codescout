---
kind: bug
status: fixed
tags:
- cluster/assertion-that-cannot-fail
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: medium
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

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live.**
`scripts/pre-commit-ledger-counts.py --json` under `--source=worktree`, `--source=index`, and HEAD's copy under
`--source=head` all return `claimed=[]`, `declared={}` with 25 `actual` entries — two of three arms are still
equality over two empty collections. `the_hook_script_agrees_on_the_cluster_parsers`
(`tests/issue_clusters.rs:1810-1853`) moved to `--source=index` in `a27b3988` without touching the empty arms: no
inert annotation, no `is_empty` assertion. The `## Reproduction` still cites `--source=worktree`. Vacuity is by
inspection; no mutation was run (`mutation-probe.sh` creates a worktree, outside the read-only brief).

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
`the_index_count_parser_discriminates` and `the_bare_n_claim_parser_discriminates` prove each is not
vacuous. So the coverage exists; what is misleading is the *live-corpus* test, which reads as a
third, independent confirmation and is not one.

The cost is therefore false confidence rather than a hole — but false confidence in a test named
`..._agrees_on_the_cluster_parsers` is the shape that stops the next person looking, which
`CLAUDE.md` names as the more expensive direction.

### Corrected 2026-10-01 — the reassurance above was wrong for one parser

The paragraph above says the two parsers are not unguarded because `the_ledger_parsers_agree_on_a_fixture` drives both across the language boundary with known non-empty answers. **For `declared` that was false.** That fixture wrote its Index rows with the slug spelled `cluster/alpha`, which is how a `**Slug:**` *declaration* is spelled, while an Index row carries the *bare* slug. Both parsers skip a row whose slug is not in the valid set, so `declared` came back `{}` from each and the fixture's comparison was also empty to empty. The Python `parse_index_counts` was covered by **no test with a non-empty answer anywhere**: a mutation reading the wrong cell survived the entire `issue_clusters` file. So the cost was not only false confidence; there was a real hole behind it.

## Suggested fix

Fixed 2026-10-01. What was done, and what was not:

- **Done, and bigger than the file expected:** the fixture's two Index rows now carry the bare slug and `declared` is pinned to the known answers (`alpha: 7`, `gamma: 2`), so the Python parser is held by a non-empty assertion for the first time. The live-corpus test's doc comment is rewritten from the measurement and the two inert arms are annotated inert at the assertion lines. This is option 1 below plus the repair the file did not know it needed.
- **Not done, deliberately:** asserting the emptiness (option 2). Equality already reds if either side starts producing output, and `no_index_row_stores_a_count` and `no_class_field_states_a_bare_n` own the claim that the corpus stores no counts, so a third copy would red twice on one cause.
- **Not done, deliberately:** dropping the arms (option 3). It would lose the ability to notice a Python-only regression against real data on a column that could return.

The original options, for the record:

1. Annotate the arms as inert, on the assertion line, saying which sibling holds each parser.
2. Assert the emptiness deliberately.
3. Drop the two arms and let the fixture test own them.

## Tests added

`the_ledger_parsers_agree_on_a_fixture` now asserts `declared` equals the known answers for its fixture, which is red against the old fixture (`left: {}`, `right: {alpha: 7, gamma: 2}`). Measured by mutation, one per site, six mutations (each parser's count cell, each `parse_bare_n_claims` field, in both languages): against the **live-corpus parity test alone** all six survive. Against the whole file, before the repair, five are killed by siblings and the Python `parse_index_counts` survives. After the repair all six are killed, the Python one by `the_ledger_parsers_agree_on_a_fixture` alone. The `actual` arm was also measured: dropping the inline-tag arm of the Python `cluster_tags` kills the live-corpus test.

The mutation runs were made on the code as committed; the doc-comment edits after them changed no code.

## Fix provenance

- **SHA:** `fcd2758cc2e4750ae7a6544cc38b5d5130783f2b` (`experiments`)
- **patch-id:** `9e85209f787eeca6daea5e3db512d70cdb75e376`

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
