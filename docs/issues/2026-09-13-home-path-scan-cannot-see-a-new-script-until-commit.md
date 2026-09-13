---
id: '40a822bad321df16'
kind: bug
status: open
title: 'BUG: the home-path scan cannot see a new script until the commit that publishes it'
tags:
- cluster/selector-narrower-than-its-population
- gates
- shared-checkout
topic: gate coverage boundaries
---

# BUG: the home-path scan cannot see a new script until the commit that publishes it

## Summary

`no_tracked_script_hardcodes_a_personal_home_path` (`tests/committed_paths.rs:170`) scans files
**tracked by git**. A new script is untracked for its entire authoring life, so the scan returns
green over it — not "clean", but "not looked at" — and only starts examining it at staging time, a
window of seconds before the commit that makes the defect everyone's problem.

The exclusion is deliberate and has its own passing control
(`an_untracked_script_is_excluded_and_a_tracked_one_is_not`). What is undocumented is the
consequence: **a new file receives zero coverage from this scan for as long as anyone would
naturally review it.**

## Symptom (Effect)

Measured 2026-09-13. `scripts/architecture-boundary-probe.py` carried
`default=Path(shutil.which("codescout") or "/home/marius/.cargo/bin/codescout")` at line 1373 from
its authoring that morning. Throughout that time:

- the full gate was green for every session in the checkout;
- two sessions reviewed the file — one of them line-by-line against the tracker's numbers, verifying
  five SHA-256s, six commit SHAs and a 315-edge matrix — and neither saw it;
- commit `d3a2c24f` published it, and the shared build went red on the next run, for every session
  in the tree.

Nobody's review was careless. The instrument that names this defect could not reach the file.

## Reproduction

1. Create an untracked script under `scripts/` containing a literal `/home/<user>` path.
2. `cargo test --test committed_paths` → **5 passed**. The file is invisible.
3. `git add` it, re-run → `no_tracked_script_hardcodes_a_personal_home_path` FAILS.

The transition happens at `git add`, not at authoring and not at review.

## Environment

codescout `experiments` @ `d3a2c24f`. `tests/committed_paths.rs`.

## Root cause

The scan's population is "tracked by git (staged or committed)". That is the correct population for
its stated purpose — a scratch script in someone's working tree should not red a shared build, and
the control test pins exactly that. But the population also excludes the state in which a **new**
file spends all of its reviewable life.

For a modified tracked file the guard is a true pre-commit check. For a new file it is a
post-hoc detector wearing the same name.

## Evidence

The failure message states the boundary plainly, and states it to the wrong audience — a reader who
is already staring at a red:

> Every path above is tracked by git (staged or committed). An untracked file is not scanned, so a
> scratch script in your working tree cannot be the cause.

That sentence is exactly right and arrives exactly too late. Nothing says the inverse at the moment
it would help: *a new script gets no coverage from me until you stage it.*

## Hypotheses tried

1. **Reviewer oversight.** **Refuted** — reproduced above; the scan returns 5-passed over a file
   containing the literal string. No amount of care recovers a file the instrument excludes.
2. **The four-command gate would have caught it pre-commit.** **Refuted, and this is the sharp
   part**: running the gate before staging cannot catch it either, for the same reason. Only the
   order `git add` → gate → commit catches it, and the natural order is `git add` → commit.

## Fix

Not implemented. Options, cheapest first:

1. ~~**Scan staged-or-untracked-under-`scripts/` in the pre-commit hook**, separately from the
   tracked-only test.~~ **WITHDRAWN 2026-09-13, and the withdrawal is the most useful thing in this
   file.** It is half redundant and half forbidden:
   - *Staged is already in scope.* `git ls-files` lists index **entries**, so a file enters the
     population the moment it is `git add`-ed — strictly before any commit exists
     (`tests/committed_paths.rs:154-158`, which says so outright: *"narrowing the population from
     the filesystem to the index loses no catch"*).
   - *Untracked is deliberately excluded, and scanning it was itself a filed defect.*
     `docs/issues/archive/2026-09-11-the-committed-scripts-gate-scans-the-filesystem-so-an-untracked-file-reds-it.md`
     records the gate walking the filesystem with `std::fs::read_dir`, so **any** session's
     in-progress scratch probe reddened the shared build for every other session, under a message
     telling them their committed scripts were broken. The fix was this exclusion. Proposing to undo
     it would reintroduce that defect **two days after it was closed**.

   The author of this file proposed it without knowing either fact. That is what a rejected approach
   costs when it is not findable from the place a reader lands, and it is the argument for keeping
   this bullet struck through rather than deleting it.

2. **Say the boundary in the passing direction — now the only live option.** The exclusion is
   documented three times in `tests/committed_paths.rs` (`:76-78`, `:121-127`, `:154-158`) and in
   the failure message, all of which are read *after* something has gone wrong or by someone already
   editing the gate. Nothing surfaces it to an author writing a new script. A line in
   `CONTRIBUTING.md` or the script template is the § *Observer Blindness* "move the scope to the read
   surface" remedy and costs nothing.

3. Leave it, and accept that a new script is uncovered until it is staged — a window of seconds in
   the normal flow, and a morning in the flow that actually happened.

**The claim this file still makes, narrowed by the above:** the blind window is **untracked**, not
*until commit*. `git add` closes it, which makes the ordering `git add` → gate → commit sufficient
and the natural `git add` → commit ordering the whole defect. The title overstates it by one step
and is left unrenamed deliberately — `id = sha256(abs_path)`, so a rename mints a new id and strands
this record's citations for a wording fix.

## Tests added

None. Note the existing control (`an_untracked_script_is_excluded_and_a_tracked_one_is_not`) is
**inert with respect to this defect** — it asserts the exclusion happens, which is the mechanism, not
the consequence. It will stay green under every fix above.

## Workarounds

`git add` new scripts early, so the guard's population includes them while you are still reviewing.

## Resume

Decide between fix 1 and fix 2. Also worth an `OB-N`: the party who structurally cannot see this is
the **author of a new file**, because the only instrument that names the defect is blind to their
file by construction, and the party who can see it is whoever runs the suite *after* the commit — a
different session, at a moment when it is already shared. That is the OB admission test met
squarely: "was careless" is disqualifying, "holds the parameter that would reveal it" qualifies, and
here nobody held it.

## References

- Observed, diagnosed and reported by sessionId `aa272bed-7d33-4e5e-bcbf-2ccf3b4c4c66`, including
  the framing carried into this file: the guard's POPULATION moved rather than the code changing, so
  the defect was unreachable by construction until the commit that exposed it.

**The triggering INSTANCE is fixed; this bug is not.** `scripts/architecture-boundary-probe.py`'s
hardcoded fallback was repaired on `experiments` at SHA
`40fb28435f96d51d8d553e7a661ce6ac0c0e342a`, patch-id
`345c535aa5b549455e34e125800523262961484f` — `--runtime-binary` now derives from `CARGO_HOME` with
a `Path.home()` fallback, and `cargo test --test committed_paths` is 5/5. That closes the one file.
The coverage gap this record is about — that a NEW script is outside the scan's population for its
entire reviewable life — is untouched by it, and the next new script inherits the same blindness.
Recording the distinction here because a fix commit sitting in a bug file is the exact shape that
gets read as closure (see the sibling defect this session filed on precisely that misreading).

- `tests/committed_paths.rs:170`
- **`docs/issues/archive/2026-09-11-the-committed-scripts-gate-scans-the-filesystem-so-an-untracked-file-reds-it.md`
  — read this before proposing any widening.** It is the inverse defect (selector WIDER than its
  name, `IC-14`'s mirror), it was closed two days before this file was opened, and its fix is the
  exclusion this file complains about. The two records bound the design from opposite sides: scan
  untracked and one session reds everyone's build; scan tracked-only and a new file is uncovered
  until staged. Neither is free, the trade was made deliberately, and this file argues only that the
  chosen side is **undocumented to authors**, not that it is wrong.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- CLAUDE.md § *Observer Blindness* — name who structurally cannot see it, who can, and the check that
  runs when nobody is worried
