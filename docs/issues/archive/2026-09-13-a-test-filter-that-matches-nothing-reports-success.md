---
id: 7998e2b1e61f1e13
kind: bug
status: fixed
title: 'BUG: a `cargo test` filter that matches nothing reports success, and the discriminator sits unused in its own output'
tags:
- cluster/selector-narrower-than-its-population
- testing
- verification
topic: verification discipline
closed: 2026-09-16
severity: med
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

**FIXED 2026-09-16** in `cb397d1f` — patch-id `4aa19437ac2dec63313831730f6f9b60f85e90b6`.

**The premise "not codescout's code to fix" ruled out the wrong thing.** `cargo test`'s
behaviour is indeed not ours to change — but the **misreading** happens in codescout's own
output surface, and this repo already ships a diagnostic for precisely that shape.
`substitution_diagnostic` names a confusing-but-correct `sh -c` behaviour that is *also* not
codescout's bug, for the same reason: the tool's output carries a plausible, self-consistent,
**wrong** explanation sitting beside the real one — `Argument list too long` there,
`test result: ok.` here. That is the third option this file's Resume did not consider, and it
reaches the *reading* error at the moment it happens, which was the stated argument against a
wrapper.

`empty_test_selection_diagnostic` in `src/tools/run_command/output.rs`: computed beside
`shell_cause`, attached at **both** response sites (the early-return buffered-summary arm and
the bottom), and rendered by `format_run_command`.

## The predicate, and why it is computed over the WHOLE output

Fires when every `test result:` summary is `ok.`, total `passed == 0`, total `ignored == 0`,
and total `filtered out > 0`.

**Never one summary line at a time.** `cargo test <filter>` builds every target in the
workspace and each prints its own summary, so a target holding no match prints
`0 passed; N filtered out` **legitimately** while a sibling runs the match. A per-line
predicate would fire on every successful filtered workspace run in this repo — and noise is
the one failure mode that gets a warning ignored rather than read. This distinction is
invisible in the single-target reproduction above, which is why the reproduction had to be
re-run against the real multi-crate workspace rather than reasoned about from the file.

**Anchored on libtest's own summary, never on command shape**, per the rule
`substitution_diagnostic` follows: an unfiltered run cannot reach `filtered out > 0`, so the
output alone proves the selection was empty and the caller's command never needs parsing.

**Two deliberate silences**, each because the right remedy differs:

- a non-`ok.` summary — a red is already loud and `wip_authors` already routes it; a second,
  quieter explanation beside a real failure is worse than none;
- a selection whose every match was `#[ignore]`d — that selection was **not** empty, and its
  remedy is `-- --ignored`, so claiming an empty selection would send the reader somewhere
  useless.

## Reach, stated because it bounds the fix

This reaches `run_command` and nothing else. A session using native `Bash` gets no diagnostic,
which `CLAUDE.md` already records as a general property of that split rather than a gap here.
Option (1) below — prefer the whole target — remains the discipline for hand-run `cargo`, and
is still the only remedy that cannot select empty by construction.

## Tests added

**Eight tests** in `src/tools/run_command/tests.rs`, fixtures taken verbatim from real
`cargo test` output measured 2026-09-16 rather than hand-written.

**This file predicted the hazard and the suite walked into it anyway.** The note here read:
*"asserting the filter matched something is an existence assertion … it needs a paired
negative … or it is satisfied by silence."* Six of the eight assertions are **silence**
assertions, and every one of them passed against the `None`-returning stub during the RED
step. Reading the warning did not prevent writing them; only mutation earned them.

## Mutation — five sites

Control 189/0 before and after each run.

| mutation | verdict | killed by |
|---|---|---|
| drop the `filtered == 0` guard | **SURVIVED**, then KILLED | `a_target_with_no_tests_at_all_is_silent` |
| drop the `passed > 0` guard | KILLED | partial-match + workspace cases (2) |
| drop the `ignored > 0` guard | KILLED | all-ignored case |
| delete the renderer line | KILLED | the compact-renderer boundary test |
| treat a `FAILED` summary as `ok.` | KILLED | failing-run case |

**The first row is the finding.** `a_full_target_run_is_silent` was written to justify the
`filtered == 0` guard and does not touch it: that fixture carries `14 passed`, so the
`passed > 0` guard refuses it **first** and the filtered guard is never consulted. **Guards in
a disjunction are evaluated in an order, and the first one to refuse an input owns it** — so a
case only exercises the guard it names if every *other* guard admits that input. The test
name, its comment and the author's intent all claimed coverage; the suite agreed by staying
green.

The isolating input is a target with an empty **population** (`0 passed; 0 filtered out`,
no filter in play), which only the `filtered == 0` guard can refuse. Same family as
`bug-fix-session-log:W-143`, reached in a different language on the same day — there a *newly
added* bound stole an older one's cases; here the overlap was present from the first draft and
nothing announced it.

**Re-run after a behaviour-preserving refactor — the same law, an hour later.** Clippy's
`question_mark` lint turned the `ok.` check from `let … else { return None }` into
`strip_prefix("ok.")?`. Behaviour is identical, so the natural move is to keep the verdicts
above. They had already decayed: **a mutation verdict is a measurement of specific BYTES**, and
M5's anchor string no longer existed — a stale script either aborts on its occurs-exactly-once
assertion, or, without one, matches nothing and reports SURVIVED, which is indistinguishable
from a real survival. Re-run against the shipped bytes: all five KILLED, control 189/0 either
side. **A refactor that preserves behaviour still invalidates the evidence that the behaviour
is guarded** — green tests after a refactor say the behaviour survived, and say nothing about
whether the guards still discriminate.

## Why the mutation was run by hand

`scripts/mutation-probe.sh` creates its worktree at `HEAD` and says so at its own line 188, so
it does **not** carry uncommitted work — and this change is uncommitted across two files,
which is the limitation already filed as
`docs/issues/2026-09-15-mutation-probe-cannot-verify-a-multi-file-uncommitted-change.md`. The
same isolation was reproduced by hand: the probe worktree reset to `HEAD`, the two modified
files copied in, each mutation asserted to occur exactly once, reverted after, with a control
run either side. The worktree was reset to `HEAD` afterwards so it holds nothing of this work.

## Workarounds

Read `filtered out: N` in the line you already have. `N > 0` with `0 passed` is the tell, and it is
in stdout on every run.

## Resume

N/A — fixed. One thing a later reader should not re-derive: the **reach limit is deliberate,
not an oversight.** This diagnostic cannot see a hand-run `cargo test` or a native `Bash`
invocation, and that is a property of where the surface sits rather than a gap in the
predicate. If it is ever extended, the `CLAUDE.md` § *Testing Discipline* law about preferring
the whole target stays the remedy for everything outside `run_command`.

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
