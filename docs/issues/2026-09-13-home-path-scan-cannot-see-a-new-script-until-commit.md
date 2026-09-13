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

1. **Scan staged-or-untracked-under-`scripts/` in the pre-commit hook**, separately from the
   tracked-only test. The hook already runs at exactly the moment the file becomes publishable.
2. **Say the boundary in the passing direction.** The control test's name asserts the exclusion
   works; nothing surfaces it to an author. A line in `CONTRIBUTING.md` or the script template costs
   nothing and is the § *Observer Blindness* "move the scope to the read surface" remedy.
3. Leave it, and accept that new scripts are caught one commit late on a branch that is never
   deleted.

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
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
- CLAUDE.md § *Observer Blindness* — name who structurally cannot see it, who can, and the check that
  runs when nobody is worried
