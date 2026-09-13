---
id: '40ca63a3b3497558'
kind: bug
status: taken
title: 'BUG: the no-stored-count gate is defeated by the repo''s own typographic house style'
tags:
- cluster/addressing-without-an-escape-hatch
- issue-clusters
- gate
- house-style
claimed_at: 2026-09-13
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
closed: null
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-02-declared-patch-ids-per-line-scan-misses-a-wrapped-value.md
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

**A live count stated in backticks passes the gate silently**, and backticks are the default
typographic choice for every figure on those lines — so the author reaching for house style
escapes a gate they did not know existed.

**REPRODUCED 2026-09-13 on the production path, and this is the part that was never
measured.** `` `n=1` `` was added to `IC-23`'s `**Members:**` — a class created that day,
whose count is genuinely current, so the claim was LIVE and not a quotation — then
`cargo test --test issue_clusters`: **24 passed**, `no_class_field_states_a_bare_n` among
them. Probe reverted.

### The original second paragraph is WITHDRAWN, and the policy text refutes it

It read: *"The ledger goes on storing the derived value the gate was inverted to remove."*
That is false, and `no_class_field_states_a_bare_n`'s own doc comment says why:

> A backticked `` `n=N` `` is untouched and still means what it always meant: a QUOTATION of
> a superseded figure, preserved with its derivation. **The migration wrapped every live
> claim in backticks rather than deleting it**, so no sentence lost its history — only its
> obligation to stay current.

Measured: **33 backticked `n=` across 21 of the 22 class files**, inside `**Members:**` and
`**Promotes to:**`. Those are the migration's deliberate output, not violations accumulating
through the hole. The digits are on disk; the obligation is not.

So the defect is **narrower and sharper** than filed: not *"the ledger is full of stored
counts"* but *"a NEW live count cannot be distinguished from the history, and the natural
way to write one escapes the check"*.

This file asserting a consequence its own gate's documentation contradicts is `OB-26` — a
§ Summary/§ Symptom stating as fact what was never checked — occurring in a bug filed about
a gate. The § Summary's mechanism claim was right throughout; only the consequence was
invented.
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

**Not implemented.** Designed and costed; the implementing change touches a shared
pre-commit hook and is held for an operator's go-ahead (see § Resume).

**The obvious fix is REFUSED by measurement, and that is the useful half.** Removing the
backtick escape reds all 33 legitimate quotations across 21 of 22 class files. That is a
campaign over a population whose coverage ratio is ~100% — CLAUDE.md § *Observer Blindness*:
*"a coverage ratio that is neither ~0% nor ~100% is a boundary someone drew before it is
drift"*, and at ~100% it is emphatically a boundary. It would also land in the ledger the
ADR names as the repo's contention head (16 sessions, 53 commits in one day), which is why
that file was split per class in the first place.

An explicit superseded-marker (`was n=11`, strikethrough, a `**Superseded:**` prefix) costs
the same 33-site sweep and buys the same thing. Rejected for the same reason.

**THE AFFORDABLE FIX IS DIFF-SCOPED, and the hook already holds every input it needs.**
`scripts/pre-commit-ledger-counts.py` reads the ledger at both `head` and `index`
(`read_ledger(source)`), and CHECK 3 already computes `members_fields()` for both. A CHECK 4
would refuse a backticked `n=<N>` that is present on a `**Members:**` / `**Promotes to:**`
line in the INDEX and absent from the same line at HEAD:

- every existing quotation is untouched forever, because its line is unchanged;
- a new live count cannot be introduced without the hook seeing it;
- no sweep, no grandfather list, no marker to remember.

That is § *Observer Blindness* position 3 in its best shape — the correct path ends in a
safe state, so compliance leaves nothing armed — rather than a rule anyone must recall.

**Precedent exists for the shape:** `a_class_gaining_a_member_names_it` is already declared
HOOK-ONLY *"because it compares the INDEX against HEAD, a question no working-tree test can
pose"*. CHECK 4 is the same kind of question, so it joins `HOOK_RULES` and is exempted on
the Rust side with that reason, which `the_hook_enforces_every_rule_it_declares` and
`every_cluster_rule_is_hook_owed_or_exempt` both police.

**Owed alongside it, and cheap either way:** the escape is currently **undocumented**.
§ *The entry shape* says only *"Never a bare `n=`"*, and nothing there or at the refusal site
defines *bare*, so a reader cannot learn that a backtick suppresses the check. CLAUDE.md
§ *Parsers Over a Namespace* prescribes exactly this when no escape is affordable: *"say so
at the refusal site — a documented limitation and a silent reinterpretation cost a reader
very different amounts."* That half needs no hook change and no sweep.
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
