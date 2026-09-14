---
id: 71af91d903e927fd
kind: bug
status: mitigated
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
and `the_index_count_parser_discriminates` — were deliberately kept when those assertions
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

### E5 — the repair was replayed against history before being built, and history refused it

`scripts/probe-backticked-count.py --replay` walks every commit that ever touched this ledger
and prints what the proposed check would have refused, with the prose around each token. Run
2026-09-13 at `41370609`, over the 334 such commits then reachable: **11 commits in 11 days, 14
tokens**, of which one reader judged 8 wrong refusals — *including the commit that filed this
bug*. Derivation in § *Fix*; re-run the script rather than citing these figures, and note that
the denominator moved by one between two runs an hour apart.

What makes this evidence rather than a second opinion: the script does not classify. It prints
the surrounding sentence and says so, because the live-or-quoted distinction is not in the
grammar — which is the same claim § *Summary* makes about the gate, now demonstrated on the
repair rather than argued about it.

### E6 — AGREEMENT is the one property that is computable, and it is rare

`--agreement` reports backticked figures equal to today's derived count: **5 of 69**,
2026-09-13. Those five read as current and will go silently wrong on their class's next member;
the other 64 differ from the corpus and are therefore visibly quotations. That ratio is what
makes an advisory possible at all — at 69 of 69 it would be a re-print of the ledger, and at 0
of 69 it would be an absence assertion pinned to an absence.
## Hypotheses tried

- *"The gate is missing a disambiguator."* **Falsified by the code** — the backtick IS
  the disambiguator, deliberately chosen after a positional rule was shown to redden two
  correct fields. Proposed by a peer session (`9716a130`) as an explicit hypothesis and
  withdrawn on reading `bare_n_values`. Recorded because the wrong framing is the
  attractive one: it points at the gate's design rather than at its interaction with
  house style.

## Fix

**Not implemented — and the held design is now FALSIFIED, not merely unshipped.** It was measured
against the corpus before being built, and the measurement refused it. What ships instead is the
documented limitation `CLAUDE.md` § *Parsers Over a Namespace* prescribes for exactly this case,
plus the probe that makes the refusal re-derivable.

**Shipped in two commits, neither of them a fix, which is why this file is `mitigated` and not
`fixed`.** The probe and this analysis are `053ebf24` (patch-id
`30f69c2396e79fe7a1fe9b2e4c088e2416cb7d03`). The ledger's documentation half landed inside
`cd138c30` (patch-id not cited: that commit's subject is the roster's mechanism column, and these
paragraphs reached it as a working-tree capture on a shared checkout rather than as part of its
change — recorded here so they stay traceable to the argument that produced them, and as one more
instance of `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md`).

**Re-derive every figure below rather than citing it:**

```
python3 scripts/probe-backticked-count.py --replay      # what the check would have refused
python3 scripts/probe-backticked-count.py --agreement   # the one discriminator that survived
```

### The sweep stays refused, unchanged

Removing the backtick escape reds all 33 legitimate quotations across 21 of 22 class files — a
campaign over a population at ~100% coverage, which `CLAUDE.md` § *Observer Blindness* calls a
boundary someone drew rather than drift, landing in the file an ADR names as this repo's
contention head. An explicit superseded-marker costs the same sweep for the same benefit.

### The diff-scoped check is REFUSED BY MEASUREMENT, and that is the useful half

The design was: refuse a backticked `n=<N>` present on a gated line in the INDEX and absent from
that same line at HEAD. Every existing quotation untouched forever because its line does not
change; no sweep, no grandfather list, no marker to remember. The shape had precedent —
`a_class_gaining_a_member_names_it` is already declared HOOK-ONLY for posing exactly this
INDEX-against-HEAD question.

Replayed over every commit that has ever touched this ledger — **334 of them, at `41370609`,
2026-09-13** — restricted to the post-inversion regime, since a bare `n=` was legal before
`1b3ac36b` and demoting one to a backticked quotation was then the *prescribed* act:

> **11 commits in 11 days**, carrying **14** newly-appearing backticked tokens.

**Both halves of that sentence carry a unit, and the denominator moved while it was being
written** — 334 was 333 an hour earlier, and an earlier pass of this same replay reported **64**
where the shipped script reports **68**, because one counted *(commit, field)* pairs and the
other counts *tokens*. Neither was wrong; they answer different questions, and they are near
enough that no reader would have queried either. That is `CLAUDE.md` § *Testing Discipline*'s
four-defensible-numbers law arriving inside the file arguing for a measurement, which is why
every figure here names its unit and its tree and why the script prints its own denominator.
Over all history the figure is 24 commits / 68 tokens; the 13 commits and 54 tokens that
separate that from the post-inversion reading are the two one-time migrations and can never
recur.

**The token count is derived and exact; the split below is an ADJUDICATION and is mine.** Whether
a token was a live claim or a quotation turns on whether its author *asserted* the number or
*mentioned* it — not in the grammar, not recoverable by any parser, and the reason the script
prints the surrounding prose and refuses to classify. One reader, reading all 14:

| verdict | n | shape |
|---|---|---|
| correct refusal — a live count written in backticks | 5 | `` **`n=16`, 2026-09-02, re-derived** `` |
| arguable | 1 | `` **Count not re-derived beyond `n=1`.** `` |
| **wrong refusal — a quotation or a mention** | **8** | `` (`n=24`, quoted here and never updated) `` |

**The decisive one is `15b0be3c` — the commit that FILED THIS BUG.** Its `**Members:**`
derivation quotes the token twice, in this sentence:

> *…so `` `n=11` `` quoting a retired figure and `` `n=11` `` asserting today's count are
> byte-identical — nothing in the grammar separates mentioning a number from making one.*

The check would have refused the commit whose prose explains the defect the check exists to
catch. That is not a tuning failure and no threshold repairs it: § *Summary*'s mechanism —
**the escape and the violation are byte-identical** — applies unchanged to any check keyed on
the same token, so the instrument inherits the blindness it was built to remove. `IC-6` holding
about the gate built to fix `IC-6`.

### What DOES discriminate, and where it belongs

Agreement with the derived count. A backticked `` `n=7` `` on a class the corpus says holds 24
is *visibly* a quotation — no reader mistakes it for current. A backticked `` `n=24` `` on a
class holding 24 reads as today's count whether or not it is one, and goes silently wrong on
that class's next member. Agreement is the hazard state, and it is rare enough to be an
advisory rather than a re-print of the ledger: **5 of 69** backticked figures, 2026-09-13.

It ships as `--agreement`, on a **read** path, refusing nothing. That placement is the finding,
not a hedge: on the commit path a false positive costs a peer their commit in this checkout's
contention head, and the replay above says most positives would be false. On a read path the
same instrument costs a glance. Both modes print their **denominator**, because every reading
here is an existence report over a parse — a parser that silently matched nothing would return
a clean empty list, and `0 of 0` is visibly broken where a bare `0` is reassuring. There is no
test behind that script; the denominator is the vacuity guard.

### The documentation half, and a correction to this file

`## The entry shape` now defines *bare* and states the limitation at the point where a field is
**written**. **The earlier claim here that the escape was undocumented *at the refusal site* was
FALSE** — `scripts/pre-commit-ledger-counts.py`'s CHECK 1 has said *"wrap it in backticks. A
backticked `n=N` is a quotation and is deliberately not checked"* since `1b3ac36b`, the inversion
commit itself, and `no_class_field_states_a_bare_n`'s assertion message carries the same
sentence. So did this ledger's `## Index` blockquote.

Two things are worth more than the correction. **First, why the claim was wrong**: it was an
*absence* claim — "nothing defines bare" — checked by reading one surface and not the other two.
Monotone under exactly the failure that occurred, which is § *Testing Discipline*'s first law
arriving at a sentence in a bug file rather than at an assertion.

**Second, why the remaining gap is real anyway, and is the whole point of the paragraph that
shipped.** All three surfaces that document the escape are *refusal* surfaces — reached only by
someone who already wrote a **bare** `n=` and was stopped. The author who reaches for a backtick
unprompted trips nothing and reads none of them. The escape was documented to everyone except
the population that uses it, which is § *Observer Blindness* position 3 verbatim: the bound was
published to an audience that never opens that surface, so the fix is to **move it to the read
surface**, not to write it again.
## Tests added

**None, and the absence is a finding rather than debt.** No regression test exists because no
production behaviour changed — the check that would have needed one was refused by measurement.
What stands in its place is `scripts/probe-backticked-count.py`, whose two modes each print a
**denominator**, so a silently-broken parser reads as `0 of 0` instead of as a clean ledger.
Both modes exit non-zero on that shape rather than reporting it quietly.

Nor is the shipped documentation testable in the way that would matter. `CLAUDE.md`
§ *Testing Discipline* already records the ceiling — pinning prose reds on every rewording, and
a shape assertion buys ARRIVAL, never ANSWERABILITY. Here it would buy even less: the
paragraph's entire claim is about *which surface a reader reaches*, and no assertion in this
repo can observe a reader.
## Workarounds

Do not write a live count in this ledger at all. Cite
`python3 scripts/probe-cluster-census.py`, which is what the policy asks for and what
makes the sentence undecayable.

## Resume

**Status `mitigated` — not `fixed`, not `wontfix`.** The mechanism is untouched and still
reachable: a live count written in backticks passes the gate today exactly as it did when this
was filed. What changed is that the escape is now stated where a field is *written*, and the one
computable discriminator ships as an advisory. `wontfix` would misreport that as a decision not
to act; `fixed` would misreport a documented limitation as a closed hole.

**Do not rebuild the commit-path check.** That is the durable half of this file, and it is
phrased as an instruction rather than a conclusion for a specific reason: the design is genuinely
attractive — bounded, precedented by `a_class_gaining_a_member_names_it`, no sweep, no marker —
and it will look free to the next reader exactly as it looked free to this one, twice. Run
`--replay` first. It takes seconds and it is the whole argument.

**Left open deliberately, and named so it is not discovered as a surprise:** `--agreement` runs
from nothing. It is wired to no gate, no hook, and not to `scripts/probe-cluster-census.py` —
which is the surface a reader of these counts actually opens. That is `declared-not-wired`
waiting to happen, and it is left rather than fixed in passing because the census probe is this
ledger's shared read path: folding a second advisory into it is a change to a surface every
session reads, not a tidy-up. The original filing question — *where does instance 2 land* —
is unchanged and still the reason this is a bug file rather than a session-log friction: the
trigger is repo-wide house style, so the next accidental escaper will be in a different work
stream and will never read this session's log.
## References

- `tests/issue_clusters.rs` — `bare_n_values`, `parse_bare_n_claims`,
  `no_class_field_states_a_bare_n` (renamed from
  `every_bare_n_in_a_class_field_matches_the_corpus` when the assertion inverted).
- `89697a15` — backticked the corpus's historical quotes, creating the escape.
- `489715ef` — the commit that corrected this session's accidental use by hand.
- `IC-6`, `cluster/addressing-without-an-escape-hatch`. The inverse direction of the
  class's usual complaint: the escape exists and is documented, and its cost is that an
  author who never intended to escape does so silently.
