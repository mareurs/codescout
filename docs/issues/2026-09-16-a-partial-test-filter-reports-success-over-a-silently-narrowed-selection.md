---
id: '81d2cdcbbdfc03a6'
kind: bug
status: fixed
title: A partial test filter reports success over a silently narrowed selection
tags:
- run_command
- testing
- diagnostics
- cluster/selector-narrower-than-its-population
---

# BUG: a partial `cargo test` filter reports success over a silently narrowed selection

## Summary

`7998e2b1e61f1e13` fixed the case where a filter matches **nothing**:
`empty_test_selection_diagnostic` names it rather than letting `ok` stand. That
fix does not generalise to a filter naming **several** tests where only some
resolve, and the guard is silent there by construction —
`src/tools/run_command/output.rs:129` returns `None` whenever `passed > 0`.

So a caller who asks for N tests and gets M < N is told nothing. The run is
byte-identical to a correct selective run of M.

**This is not a criticism of that predicate.** `passed > 0` is deliberate and
tested (`a_selection_that_matched_some_is_silent`): a filter matching 3 of 14 is
the commonest shape there is, and firing on it would produce a warning everyone
learns to ignore. The defect is that a remedy now exists, reads as covering
"did my filter do what I asked", and covers only the total-miss half — which is
what stops the next person looking.

## Symptom (Effect)

A verification step that names a renamed, deleted or misspelled test **alongside
live ones** certifies itself. The dead name contributes nothing and costs nothing
visible.

## Reproduction

Through `run_command`, 2026-09-16, on this checkout:

```
cargo test --lib -- check_row_behind_file_abstains_on_an_empty_stored_hash \
                    zzz_no_such_test_anywhere_at_all
```

Two names requested, one bogus. Observed:

```
running 1 test
test ... check_row_behind_file_abstains_on_an_empty_stored_hash ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5617 filtered out
exit_code: 0
```

No `empty_test_selection` field in the response. The control is the sibling case
the existing fix owns: drop the real name and the diagnostic fires.

## Environment

codescout `experiments`, 2026-09-16, at `cbe62fa7`.

## Root cause

**The discriminator is not in the output, which is the whole difference from
`7998e2b1e61f1e13`.** There, `filtered out: N` beside `0 passed` was present on
screen and unread — the bug file says so in as many words, and the remedy was to
read what was already there. Here libtest reports a single `filtered out` total
and never reports *which of the supplied filters matched nothing*, so no amount
of parsing its output recovers the fact. A remedy has to come from somewhere
else.

That asymmetry is why the first fix could be a parser and this one cannot.

## Evidence

Found by `codescout-09` (sessionId `e5691fad-9f78-4cd1-ad14-edfdd1fee41f`)
without the tool's help, while verifying that a rename of mine had retired its
old assertion rather than disabling it. They filtered on four test names, saw
`3 passed`, and concluded the fourth selected nothing — **by counting the tests
that ran, not by reading the status**, which would have said `ok` either way.

They then corrected their own account of it: they counted because they had read
`cb397d1f` that morning and were primed to, not from independent insight. That
correction is what makes this a bug report rather than an anecdote — the
discipline that caught it is a policy held by one reader on one day, and the
mechanism that exists does not reach this case.

**Independently reproduced, and recorded because a confirmation is a
denominator.** `e5691fad` ran the same shape through `run_command` at current
HEAD with a different pair of filter names and got the same result — `1 passed;
5617 filtered out`, exit 0, no diagnostic. That is published here rather than
absorbed: a re-derivation which *confirms* goes on record, because a population
whose confirmations are invisible looks self-correcting. It is not a second
catch.

They also checked the *"discriminator is genuinely absent"* claim against that
output rather than agreeing with it: `5617 filtered out` is one aggregate, and
nothing in libtest's output attributes it to either supplied filter.

## Hypotheses tried

1. **Hypothesis:** the existing diagnostic would have caught it if run through
   `run_command` rather than native `Bash` — i.e. a tool-choice miss rather than
   a gap. **Test:** the reproduction above, through `run_command`.
   **Verdict:** rejected, and it inverts the hypothesis. `passed > 0` short-
   circuits at `output.rs:129` before any per-filter reasoning, so the channel
   makes no difference.

## Fix

**FIXED 2026-09-19** — `f8cc41aaf4e8836174d0ccfe7371ac3cce9a6eee`, patch-id `519482a911f0ed2089d596d13e552cac0e6a61ce`. `partial_test_selection_diagnostic` runs `cargo test -- --list`, diffs the supplied filter names against it, and NAMES any that matched nothing. `passed > 0` is untouched, per this file's own prohibition. The extra subprocess is gated to the shape the bug is about — command contains `cargo test`, a standalone `--` is present, and at least TWO non-flag tokens follow — so an ordinary single-filter or filter-less run never pays it. Split `..._with(lister)` for testability, mirroring `attribution.rs`'s existing pattern, so no test spawns a real cargo.

Not implemented, and the cheap parser route is unavailable for the reason under
*Root cause*.

The one candidate that would work: `cargo test -- --list` enumerates test names
cheaply and without running them, so each supplied filter can be checked against
that set and the non-matching ones named.

**Cost, measured rather than adjectival.** `cargo test --lib -- --list` at
`109c5b93`, 2026-09-16T14:20:01Z, emits **5,618 test names** in 5,620 lines (one
blank, one `5618 tests, 0 benchmarks` summary). The unit is *names*, not lines,
and the two differ by two — stated because an earlier account of this figure said
"lines". Derived twice independently: `e5691fad` measured it first and this
reading re-derived it at the same HEAD. So the price is one extra invocation plus
a 5,618-name set to intersect against, per run, on every `cargo test` the tool
wraps — which is why this is a proposal rather than an obvious yes. The check
would also have to hold its own population honestly, since `--list` output is
per-target.

**Do not answer it by relaxing `passed > 0`.** That predicate is load-bearing
and its test says why: a correct selective run is the commonest shape, and a
diagnostic that fires on it is a warning that gets ignored — which would cost
more than this bug does.

## Tests added

None yet.

## Workarounds

Count the tests that ran against the number of names you supplied. The status
line cannot distinguish them; `running N tests` can.

## Resume

Sibling and prior art: `docs/issues/archive/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md`
(`7998e2b1e61f1e13`, fixed in `cb397d1f`), whose remedy this bug is the
uncovered half of.
