---
id: '40ca63a3b3497558'
kind: bug
status: open
title: 'BUG: the no-stored-count gate is defeated by the repo''s own typographic house style'
tags:
- cluster/addressing-without-an-escape-hatch
- issue-clusters
- gate
- house-style
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-09-02-declared-patch-ids-per-line-scan-misses-a-wrapped-value.md
severity: low
---

# BUG: the ledger's no-stored-count gate is defeated by the repo's own typographic house style

## Summary

`no_class_field_states_a_bare_n` (`tests/issue_clusters.rs`) enforces that
`docs/trackers/issue-clusters.md` stores no derived count. Its escape is the backtick:
`bare_n_values` skips any `n=` inside a complete backtick span, on the documented
grounds that the house style preserves superseded figures with their derivations rather
than overwriting them.

The escape and the violation are **byte-identical**. `` `n=11` `` quoting a retired
figure and `` `n=11` `` asserting today's count are the same characters, and nothing in
the grammar separates *mentioning* a number from *making* one.

That alone would be an ordinary limitation. What makes it a defect is the direction it
fails in: **the compliant-looking write IS the escaping write.** Every neighbouring
figure in those fields is backticked, so backticks are the default typographic choice
for any number in this ledger — an author reaching for them out of house-style habit
escapes a gate they did not know existed.

## Symptom (Effect)

A live count stated in backticks passes the gate silently. The ledger goes on storing
the derived value the gate was inverted to remove, and the failure is invisible to both
parties: the author believes house style, the gate believes a quotation.

## Reproduction

Observed 2026-09-02, n=1, by the session that wrote it:

1. Read `IC-18`'s `**Members:**`, which stated `` `n=10` `` — current-looking, dated,
   with its derivation attached.
2. Added a member and bumped it to `` `n=11` ``, backticked to match every sibling
   figure on the line.
3. Ran `cargo test --test issue_clusters` — **17 passed**, including
   `no_class_field_states_a_bare_n`.
4. Read that test's body afterwards and found the assertion had inverted about five
   hours earlier: the ledger no longer stores counts at all.

The bump was wrong on the policy and green on the gate. Corrected by hand at `489715ef`
(the field now cites `scripts/probe-cluster-census.py`), not by any check.

## Environment

`experiments`, shared checkout, ~20 codescout servers across three profiles. The policy
inverted the same day, which is what made a current-looking stored cell available to
misread.

## Root cause

One token carries two meanings and the scheme cannot express the difference:

- **Typographic** — backticks mark code/identifier voice, used everywhere in this ledger,
  including around numbers.
- **Semantic** — a backtick span tells `bare_n_values` "this is a quotation, do not
  check it".

There is no way to write a backticked number that means *a live claim*, and no way to
write an unbackticked one without breaking the ledger's voice. So the author's intent —
the only thing that distinguishes the two cases — is unrepresentable, and the gate reads
typography as consent.

Note what is **not** wrong here. The escape is deliberate and its rationale is written
out: position cannot discriminate, because two `**Promotes to:**` fields legitimately
*open* with a historical citation (*"the archive backfill took it from `n=2` to
`n=27`"*), so a first-`n=`-wins rule reddens two correct fields. The parser's own doc
comment names the backtick as the disambiguator `IC-6` says a parser over a namespace
owes — and it is right. This entry is about the cost of that escape, not its absence.

## Evidence

### E1 — the parser states the coupling itself

`bare_n_values`' doc comment opens: *"The backtick is the escape, and it is now the ONLY
signal separating a live claim from a quotation."* Read at the bytes 2026-09-02. The
mechanism is documented; the accidental-use direction is not.

### E2 — the span rule was already one shipped bug in this area

The first version tested only the byte before `n=`, so a tight `` `n=16` `` was skipped
while an `n=` inside a backticked *phrase* returned two live claims. Found by
`codescout-3e` testing the shipped parser, and the corpus was clean at the time, so no
review and no run against real data would have surfaced it. Its own comment calls this
*"`IC-14` inside the gate that polices `IC-14`"*.

### E3 — the count is published with its unit, and the units disagree

Eleven backticked `n=` occurrences under the span reading; ten under a tight-token
reading. The difference is one `` `n=1 taggable` `` — a backticked *phrase*, which is
corpus evidence that the house style already wraps prose rather than tokens.

### E4 — the paired discriminators cover this direction ZERO times, not weakly

`no_class_field_states_a_bare_n` is an absence assertion, so it is monotone under parser
failure: a `bare_n_values` matching nothing yields an empty list and passes green
forever. The two fixture-driven discriminators — `the_bare_n_claim_parser_discriminates`
and `the_index_row_parser_discriminates` — were deliberately kept when those assertions
inverted, precisely to stand against that, and they do that job: they run over
adversarial fixtures with known answers and prove the parser **can** find a bare `n=`
that is there.

What neither can reach is the opposite direction — a compliant-looking write that is
**accidentally escaped**. Proving the parser finds an unescaped `n=` says nothing about
whether an author who meant a live claim wrote one the parser will skip. So the pair is
monotone in one direction and this failure is covered **zero** times rather than weakly,
which is `CLAUDE.md` § *Testing Discipline*'s first law: two guards satisfied in the same
direction leave the property held by neither. The design note justifying the
discriminators' retention does not mention it.

Contributed by session `f13f8169`, which wrote both the inverted assertion and the note,
and which supplied this direction rather than defending the design — the author holding
the parameter is the party structurally least able to see it.

## Hypotheses tried

- *"The gate is missing a disambiguator."* **Falsified by the code** — the backtick IS
  the disambiguator, deliberately chosen after a positional rule was shown to redden two
  correct fields. Proposed by a peer session (`9716a130`) as an explicit hypothesis and
  withdrawn on reading `bare_n_values`. Recorded because the wrong framing is the
  attractive one: it points at the gate's design rather than at its interaction with
  house style.

## Fix

Not implemented, and `wontfix` is a defensible terminal state — say so explicitly rather
than leaving it open by default if that is the ruling.

Candidates, cheapest first:

- **Say it at the refusal site.** The failure message already teaches the escape
  (*"If you meant to QUOTE a figure … wrap it in backticks"*). It could also name the
  trap: *a backticked number is not checked, so do not reach for backticks out of house
  style if you mean today's count.* This reaches only authors who see a refusal, which
  is exactly the population that does **not** include this instance — the gate stayed
  green.
- **Require quotations to be marked as such.** e.g. only skip an `n=` in a span that
  also carries a superseded marker. Costs a corpus migration and re-opens the positional
  problem for the two legitimate opening citations.
- **Nothing.** The forcing function moved to `scripts/pre-commit-ledger-counts.py`
  (member prose, not numbers), so the stored count matters less than it did; the residual
  is a stale figure nobody is obliged to update.

## Tests added

None — capture-on-notice record, n=1.

## Workarounds

Do not write a live count in this ledger at all. Cite
`python3 scripts/probe-cluster-census.py`, which is what the policy asks for and what
makes the sentence undecayable.

## Resume

Open, n=1, and the disposition question is *where does instance 2 land* rather than
whether the trade-off was right. Filed as a bug file rather than a session-log friction
for exactly that reason: the trigger is repo-wide house style, so the next
accidental-escaper will be in a different work stream and will never read this session's
log. A cluster tag is the only surface where n=2 finds n=1. Argued by peer session
`9716a130`; the deciding authority is `CLAUDE.md` § *Bug Tracking* — *"Open a bug file
for ANY bug noticed during work — including incidental bugs we won't fix and tool
quirks."*

## References

- `tests/issue_clusters.rs` — `bare_n_values`, `parse_bare_n_claims`,
  `no_class_field_states_a_bare_n` (renamed from
  `every_bare_n_in_a_class_field_matches_the_corpus` when the assertion inverted).
- `89697a15` — backticked the corpus's historical quotes, creating the escape.
- `489715ef` — the commit that corrected this session's accidental use by hand.
- `IC-6`, `cluster/addressing-without-an-escape-hatch`. The inverse direction of the
  class's usual complaint: the escape exists and is documented, and its cost is that an
  author who never intended to escape does so silently.
