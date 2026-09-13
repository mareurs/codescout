---
id: c719e2442614336b
kind: bug
status: open
title: 'BUG: a `cargo test` filter that matches nothing reports success, and the discriminator sits unused in its own output'
tags:
- cluster/selector-narrower-than-its-population
- testing
- verification
topic: verification discipline
---

# BUG: a `cargo test` filter that matches nothing reports success, and the discriminator sits unused in its own output

## Summary

`cargo test <filter>` is a **selector over the test namespace**. A filter matching nothing selects
an empty set, and the harness then reports success over that empty set: `test result: ok.`, exit
`0`. Nothing distinguishes *"every test I asked for passed"* from *"I asked for a test that does not
exist"*, so a verification step silently becomes a no-op that certifies itself.

The discriminating field is already printed. `filtered out: N` is what separates *nothing broke*
from *nothing was looked at*, and both the tool's caller and its reader reach for the exit code
instead.

## Symptom (Effect)

Measured 2026-09-13, tree `1c7e4b48`. Three filters — a helper function's name, a name that exists
nowhere, and `--exact` on a nonexistent name — produce **byte-identical** output:

```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s
exit=0
```

A genuinely passing run of the same target prints:

```
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.56s
```

The two differ only in *which* integers are non-zero. The word `ok` and the exit code are the same.

## Reproduction

```sh
cargo test --test doc_tool_refs present_tense_surfaces      # a HELPER fn, not a test  -> 0 run, exit 0
cargo test --test doc_tool_refs zzz_no_such_test_anywhere   # exists nowhere            -> 0 run, exit 0
cargo test --test doc_tool_refs -- --exact zzz_no_such_test # --exact does not help     -> 0 run, exit 0
```

All three: `0 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out`, exit `0`.

## Environment

codescout `experiments` @ `1c7e4b48`. Stock `cargo test` / libtest. Not codescout's own defect — a
property of the harness that this repo's verification discipline runs into.

## Root cause

A name filter is a **selector**, and `cargo test` reports on the population it selected rather than
on the population the caller meant. The excluded members were never examined, so there is no count
to report and nothing to mark — `cluster/selector-narrower-than-its-population` in its plainest
form, with the selector narrowed all the way to empty.

`--exact` does not help, because the failure is not fuzzy-versus-exact matching: an exact match
against a name that does not exist is still an empty selection.

**ONE CLAIM, TWO TAILS — and the wide tail is the one nobody writes down.** A selector too NARROW
returns `0 passed; 0 failed`, exit `0`. A selector too WIDE returns a plausible pile. Measured the
same evening, thirty minutes after this file was opened: a prior-art check over `docs/issues/archive/`
was run with the pattern `fix anchor|Fix provenance` and returned **119 files** — because
`## Fix provenance` is a section heading every archived bug carries. The pattern matched the corpus's
furniture, not its content.

Neither tail returns an error. **Both are answers about the SELECTOR that get read as answers about
the CORPUS** — `0` reads as *"nothing is wrong"*, `119` reads as *"this is thoroughly covered"*, and
the true reading of each is *"ask a better question"*. The narrow tail is the one this class usually
records, because an empty result feels like information; the wide tail is more comfortable still,
which is why it goes unexamined. Narrowing the pattern to the detector's actual name is what turned
that 119 into two real neighbours (`bdc13887`).

**The pairing matters for the remedy.** "Read `filtered out: N`" answers only the narrow tail. The
general form is: *before believing a selector's result, state what the selector would return if it
were wrong in each direction* — and if you cannot name a result that would look different, the
number is about your pattern. (Second tail contributed by sessionId
`f0b1a4c7-e991-4478-bf22-b088483b6821`, who named the two as one claim.)

## Evidence

**The incident, which is the reason this is filed rather than noted.** While editing
`docs/TAXONOMY.md` I identified two tests that read that file — `reader_docs_contain_no_retired_call_forms`
and, per a `grep` annotation, `present_tense_surfaces` — and ran both before committing, explicitly
to avoid discovering a break in the gate. The first passed for real. The second was a *helper
function*, not a test, so the run matched nothing and exited `0`. I read that as verification and
was one step from committing on it.

`present_tense_surfaces` is `tests/doc_tool_refs.rs:414`, a `fn ... -> Vec<PathBuf>` used by four
actual tests. The grep tool had correctly reported it as the **enclosing symbol** of the
`docs/TAXONOMY.md` reference; reading an enclosing symbol as a test name is the caller's error, and
the harness's silence is what turns the error into a false verification.

Recovered only by noticing `14 filtered out` in output already on screen — the discriminator was
never missing, it was unread.

## Hypotheses tried

1. **`--exact` disambiguates.** **Refuted** — variant C above, byte-identical.
2. **The exit code distinguishes them.** **Refuted** — `0` in all three, and `0` for a real pass.
3. **It only bites on typos, so care is the remedy.** **Rejected on shape.** The incident was not a
   typo: the name was copied from real tool output, correctly reported, and misread by one category.
   "Be careful" does not reach a failure whose input looks right.

## Fix

Not codescout's code to fix, so the remedy is local discipline with a mechanism where one is cheap:

1. **Prefer the whole target to a name filter when verifying.** `cargo test --test <target>` cannot
   select empty — its `filtered out` is `0` by construction. This is the "correct path ends in a safe
   state" shape and costs a few seconds.
2. **If a filter is used, assert on `filtered out`, not on the exit code.** A wrapper that fails when
   `running 0 tests` appears is three lines and turns a plausible answer into an error.
3. `--exact` plus a known-present control test in the same invocation also works, at the cost of
   remembering to add it.

Not implemented here; (1) is already what this session adopted.

## Tests added

None. Note the shape any guard would need: asserting *"the filter matched something"* is an
existence assertion and is monotone under the harness being replaced by one that prints nothing, so
it needs a paired negative — a deliberately-empty filter that MUST be reported — or it is satisfied
by silence.

## Workarounds

Read `filtered out: N` in the line you already have. `N > 0` with `0 passed` is the tell, and it is
in stdout on every run.

## Resume

Decide whether (2) is worth a `scripts/` wrapper or belongs in `CLAUDE.md` § *Testing Discipline* as
a law rather than a mechanism. Argument for the law: the failure is a *reading* error at the moment
of interpreting output, which no wrapper reaches when someone runs `cargo test` by hand.

## References

- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- **Framing by sessionId `f0b1a4c7-e991-4478-bf22-b088483b6821`**, whose sentence is the general
  statement this file is an instance of: *"a test filter is a selector over a namespace, and a
  selector that matches nothing returns the same green as a suite that passes."* They declined
  authorship on the grounds that the party holding the reproduction should own the file — *"a bug
  file authored by someone who only heard about the repro is a hypothesis with a filename"* — which
  is itself worth keeping.
- CLAUDE.md § *Testing Discipline* — *assert on the name, not on a proxy for it*: the discriminator
  is usually already in the output, unused, while both parties reach for a number.
