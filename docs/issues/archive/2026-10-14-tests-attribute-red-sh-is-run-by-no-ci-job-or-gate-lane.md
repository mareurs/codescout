---
id: f580495df895c775
kind: bug
status: fixed
title: 'BUG: tests/attribute-red.sh (43 cases) is run by no CI job and no gate lane, so its suite sat 5 cases red for two days with every gate green'
owners:
- marius
tags:
- cluster/declared-not-wired
topic: authorship attribution at a red
closed: 2026-10-16
opened: 2026-10-14
owner: marius
severity: medium
---

# BUG: tests/attribute-red.sh (43 cases) is run by no CI job and no gate lane, so its suite sat 5 cases red for two days with every gate green

## Summary

`tests/attribute-red.sh` is the regression suite for `scripts/attribute-red.py`, the engine that names who wrote a dirty file at a red. Nothing runs it. `.github/workflows/ci.yml` and `scripts/gate.sh` do not mention it, so a defect in the engine cannot fail a build. Found while fixing `docs/issues/archive/2026-10-13-attribute-red-compares-a-raw-session-id-with-qualified-owners-so-your-own-write-reads-as-a-peers.md`.

## Symptom (Effect)

The suite passed 36 and failed 5 at `5ae71b35`. It had been red since `eed25579` (2026-10-11), and every gate in that window was green, because no gate runs it. The 2026-10-13 fix made it pass again, and nothing re-checks that.

## Reproduction

At HEAD, search the tree for a runner:

```
grep -rn 'attribute-red\.sh' --include='*.yml' --include='*.sh' --include='*.py' --include='*.toml' --include='*.rs' .
```

The only hit is a code comment, `src/tools/run_command/attribution.rs:532`. Measured 2026-10-14. The `shell-tests` job (`.github/workflows/ci.yml:99-115`) runs `hooks-discrimination.sh`, `file-provenance.sh` and `peer-sessions.sh` only.

## Environment

Linux, `experiments` at `388416f2`. The suite passes by hand in this checkout: `bash tests/attribute-red.sh` gave `passed=43 failed=0` on 2026-10-14. It passes on `ubuntu-latest`: the job `attribute-red.sh (red attribution engine)` concluded `success` in CI run 38848320964 at `59e43edc`, which contains `4f5b16e4` (measured 2026-10-16 with `gh run view`).

## Root cause

The suite was written without a runner and no rule requires one. `ci.yml:73-98` records the same defect for three other suites on 2026-09-01 and names its cost: a suite nothing runs is evidence about one instant. Measured 2026-10-14 by the grep above.

## Evidence

- The fixed bug above, section Tests added: 36 passed / 5 failed at `5ae71b35`; 41 / 41 and then 43 / 43 after `3fb612b9`.
- `ci.yml:117-121`: `shell-tests` is a known-flaky lane, so new suites get their own job, appended at the end of the file so no existing `ci.yml:<line>` citation shifts.

## Hypotheses tried

None. The cause is the missing runner; there is nothing to hypothesise.

## Fix

Implemented 2026-10-14 in `4f5b16e4` (`experiments`): the job `attribute-red-tests` at the END of `.github/workflows/ci.yml` runs `bash tests/attribute-red.sh`, with its own comment, following `gate-slot-tests`. The job configures no git identity: `env -i PATH=... HOME=<empty dir> GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null bash tests/attribute-red.sh` passed every case (45 of 45) on 2026-10-14. That was a local simulation of the runner. The first CI run of the job is green: run 38848320964, job 116803860689, `success`, completed 2026-10-16T04:00:42Z. The suite ends with `[ "$FAIL" = "0" ]`, so a failing case gives a non-zero exit and fails the job; this was read in `tests/attribute-red.sh:343` and no red run was provoked in CI.

Not done: `scripts/gate.sh` still does not run the suite. Adding it there is a separate decision, because the gate's own tests are the ones other sessions wait on.

## Fix provenance

- **SHA:** `4f5b16e4730592c662d5b9e484bbc33b35971120` (`experiments`; the same commit fixes `docs/issues/archive/2026-10-14-a-failure-of-the-red-attribution-helper-is-discarded-twice-and-reads-as-nothing-to-say.md`)
- **patch-id:** `bb101f16a080b6ba3f57381953117d7d57f92fca` (`git show 4f5b16e4 | git patch-id --stable`)

## Tests added

None. The defect is the missing runner; the suite itself already exists.

## Workarounds

Run `bash tests/attribute-red.sh` by hand before changing `scripts/attribute-red.py`, `scripts/file-provenance.py` or `scripts/session_context.py`.

## Resume

Closed 2026-10-16. The first CI result of `attribute-red-tests` was green. Nothing remains for this bug. Whether `scripts/gate.sh` should also run the suite is a separate decision and is not tracked here.

## References

- `tests/attribute-red.sh`, `scripts/attribute-red.py`
- `.github/workflows/ci.yml:73-115`
- `docs/issues/archive/2026-10-13-attribute-red-compares-a-raw-session-id-with-qualified-owners-so-your-own-write-reads-as-a-peers.md`
