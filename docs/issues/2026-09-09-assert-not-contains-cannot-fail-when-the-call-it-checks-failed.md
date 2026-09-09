---
kind: bug
status: open
tags:
- cluster/assertion-that-cannot-fail
closed: null
opened: 2026-09-09
owner: marius
related: []
severity: low
---

# BUG: `assert_not_contains` cannot fail when the call it checks failed

## Summary

Both MCP smoke scripts turn a failed `mcp call` into an **empty** `RESULT`, and both
define an `assert_not_contains` helper that greps that variable. An absence assertion
over an empty string is satisfied by construction, so any future use of the helper
yields a check that passes precisely when the server is unreachable. The helper is
currently called **zero** times in either script, so this is a latent trap rather than a
live false pass — filed because the next person to reach for it gets a green tick from a
dead server, and nothing in the script says so.

## Symptom (Effect)

No observed failure. The shape, at `tests/mcp-smoke-rust.sh:18` and `:25` (tree
`2438a645`):

```sh
RESULT=$(mcp call "$tool" -p "$params" ce-test 2>/dev/null) || RESULT=""
...
assert_not_contains() {
    ! echo "$RESULT" | grep -q "$1"
}
```

`tests/mcp-smoke-kotlin.sh:24` and `:31` are the same two lines against the `ce-kt-test`
alias.

## Reproduction

Not run — these scripts need the `mcp` CLI and a live server, and neither is invoked by
CI (`.github/workflows/*.yml` names no `mcp-smoke` target; both file headers say *"Run
from the project root"*). The shape is read from the source rather than executed:
`inferred from tests/mcp-smoke-rust.sh:18,25 — not measured`.

Derived 2026-09-09 at tree `2438a645`: `assert_not_contains` appears **twice** across
`tests/mcp-smoke-*.sh`, both times as a definition, **zero** times as a call.

## Environment

Manual-only shell scripts; no CI lane. Branch `experiments`.

## Root cause

Two mechanisms that are each defensible alone and unsafe together.

**The recording discards the refutation.** `2>/dev/null` on the `mcp call` throws away
the only artifact that distinguishes *"the server answered and the string is absent"*
from *"there was no answer"*. That is § *Testing Discipline*'s second law — a check
cannot detect what its recording filters out — and widening the sample does nothing
against it.

**The fallback supplies a plausible value rather than a status.** `|| RESULT=""` is not
a control-flow exit; it produces a well-formed value that flows into the assertions. The
positive helper `assert_contains` is safe under it — an empty `RESULT` fails every
substring grep, loudly. The negative one is not: absence assertions are **monotone under
removal**, and an empty string is the maximal removal. So the two helpers are blind in
opposite directions over the same variable, and the pair reads as symmetric coverage.

## Evidence

### The pair is asymmetric, and only one half is safe

`assert_contains` over `RESULT=""` → grep finds nothing → assertion fails → the dead
server is reported. `assert_not_contains` over `RESULT=""` → grep finds nothing → `!`
inverts → assertion passes → the dead server is reported as a passing check.

### How it was noticed

Not by reading these scripts for their own sake. A peer session (sessionId
`c9ab2c8d-dd74-43f4-9940-25756379a312`) reported a probe of their own that returned
`unresolvable` for three peers because a `python3 -c` `NameError` was eaten by
`2>/dev/null` while `|| echo "unresolvable"` supplied a well-formed answer — and a
**real** documented hazard they had read that morning (peer pid decay) made the broken
instrument's output credible. Their tell is the transferable part and is worth more than
this bug: **three-for-three unresolvable is a claim about the probe, not the
population.** A uniform result across a heterogeneous population is a statement about the
instrument.

That prompted a scan of this repo's own instruments for the shape. **Name the selector,
not just the number:** `grep` for `2>/dev/null` followed by a `||` fallback **on the same
line**, over `scripts/**` and `tests/*.sh`, derived 2026-09-09 at tree `2438a645`. A
fallback split across lines is outside it, so **31** is a count of that list and not of
the corpus.

The 31 sites, in 14 files, reconcile as:

| n | shape | verdict |
|---|---|---|
| 27 | control flow on a genuine condition — `git rev-parse … \|\| exit 0` outside a repo, `readlink /proc/<pid>/exe \|\| continue` on a dead pid, `\|\| true` in a cleanup trap | correct |
| 2 | `\|\| echo "?"` / `"(unreadable)"` (`scripts/peer-sessions.sh:179-180`) — the fallback is *visibly* an unknown in the rendered table, not a plausible value | correct, and the shape to copy |
| 2 | `\|\| RESULT=""` (`tests/mcp-smoke-rust.sh:18`, `tests/mcp-smoke-kotlin.sh:24`) | **this bug** |

The discriminator that sorts them is whether the fallback yields a **reported value**
where the swallowed error could be a *code defect* rather than an expected absence.

**One candidate was checked and cleared, and that is recorded rather than dropped.**
`scripts/install-hooks.sh:327` reads `seeded="$(grep -c . "$seed_log" 2>/dev/null || echo
0)"`, which looks like a failure becoming the number `0`. It is not: `grep -c` exits **1**
on an empty file while printing `0`, so the fallback normalizes an exit status into the
number it already means, and `$seed_log` is a file the same script writes two lines above
(`:325-326`). Counted in the 27. Publishing the clear as well as the find is the point —
a re-derivation that confirms is a **denominator**, and absorbing it as a catch makes the
corpus look self-correcting.

## Hypotheses tried

1. **Hypothesis:** the helper is used somewhere and this is a live false pass.
   **Test:** grep both scripts for call sites.
   **Verdict:** rejected — zero calls; it is a definition-only trap. Severity dropped to
   `low` on that basis, and the file kept rather than discarded, because "unreached" is a
   property of today's corpus and the helper exists to be reached.

## Fix

**Not attempted, deliberately.** The obvious repair — guard the helper on a non-empty
`RESULT`, or make `call()` exit rather than blank the variable — cannot be given an
observed RED here: running these scripts needs the `mcp` CLI and a live server, and
asserting against a re-implementation of the helper in a harness is the second-level
antipattern § *Testing Discipline* names. A blind fix to a script nothing runs would buy
a green tick and no evidence. Left open for a session that has the server up.

## Tests added

None. See § Fix — the environment to demand a RED in is not available here, and the
script is not in a CI lane that could carry one.

## Workarounds

Do not use `assert_not_contains` in these scripts. If a negative check is genuinely
needed, assert first that `RESULT` is non-empty — the discriminator is already in the
variable, unused.

## Resume

Two options for whoever has a live server, in preference order. (1) Delete
`assert_not_contains` from both scripts: it has no callers, and a helper that is unsafe
in its only possible use is worth less than its absence. (2) If it is wanted, make
`call()` fail loudly — drop `2>/dev/null`, and on a non-zero exit print the error and
`exit 1` rather than blanking `RESULT`. Demand an observed RED by pointing the alias at a
dead server and confirming the script reds instead of passing.

## References

- `tests/mcp-smoke-rust.sh`, `tests/mcp-smoke-kotlin.sh`
- `docs/trackers/issue-clusters/IC-16-assertion-that-cannot-fail.md`
