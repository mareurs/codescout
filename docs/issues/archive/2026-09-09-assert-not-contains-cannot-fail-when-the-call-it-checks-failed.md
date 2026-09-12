---
kind: bug
status: fixed
tags:
- cluster/assertion-that-cannot-fail
claimed_at: 2026-09-12
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
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

> ### RETRACTED 2026-09-09 — the clearing below is WRONG, and a wrong clearing is worse than none
>
> `grep -c .` prints `0` on an empty file **and** exits 1. `||` does not *replace* stdout,
> it *appends* — so `seeded="$(grep -c . "$seed_log" 2>/dev/null || echo 0)"` yields
> `$'0\n0'`, and the line renders as `0` / `0 inherited pair(s) marked unknown` across two
> lines. Measured directly: `seeded=$'0\n0'`.
>
> I had the mechanism right and drew the opposite conclusion from it. The paragraph below
> correctly states that grep prints 0 and exits 1, then concludes the fallback "normalizes
> an exit status into the number it already means" — which would be true of a fallback that
> substituted, and this one adds.
>
> **Filed independently as `d81efeef5252bfcc` by another session**, who found it after I had
> published it as clear. That is the cost of the error and the reason it is retracted in
> place rather than edited away: § *Testing Discipline* asks that a re-derivation which
> confirms be published, because a confirmation is a **denominator**. A confirmation that is
> *wrong* is not a denominator, it is a **foreclosure** — a finding invites the next reader
> to check, a clearing tells them not to. Under-counting the 27 would have cost nothing;
> mis-clearing one site cost someone else the rediscovery.
>
> The site count in the table above is therefore **26 correct, not 27**, and this is the
> third shape in that table rather than a member of the first.

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

Fixed, taking § Resume's option (1) — `assert_not_contains` is **deleted** from both
scripts — plus the mechanized half this file said it could not have.

**The premise was re-derived at HEAD, not cited.** This file's "zero call sites" was
measured at tree `2438a645`; both scripts were edited since (`9406f3c4`, same session as
this fix). Re-checked before deleting: still exactly one definition per script
(`mcp-smoke-rust.sh:25`, `mcp-smoke-kotlin.sh:31`), still zero calls.

In each script's place is a comment stating why there is no such helper, what makes
`assert_contains` safe under the same fallback when this one is not, and what to do
instead (`assert RESULT is non-empty FIRST`).

**§ Fix's blocker was real for `call()` and does not apply to a static gate — that is the
substantive correction this fix makes to the file.** The reasoning for filing unfixed was
sound: repairing `call()` needs the `mcp` CLI and a live server, no CI lane carries these
scripts, and asserting against a re-implementation of the helper in a harness is the
second-level antipattern. All true. But the *shape* can be forbidden statically with no
server at all, and that assertion can be driven RED — which is the evidence § Fix
correctly refused to ship without.

`no_smoke_script_asserts_absence_over_a_blankable_result`
(`tests/mcp_smoke_scripts_reference_real_tools.rs`) matches a negated grep over `$RESULT`.
**Both branches driven, 2026-09-12:**

| probe | result |
|---|---|
| the deleted helper re-added as CODE | **RED**, naming `tests/mcp-smoke-rust.sh:38` and printing the remedy |
| the byte-identical line appended as a `#` COMMENT | green — the comment skip is real, not decorative |

The second row is the one worth keeping: documenting a banned shape is the likeliest way
the next person writes it, so without the skip the gate would red on its own explanation.
It is a distinct branch of the predicate and was mutated separately, per CLAUDE.md's
"mutate once per guarded SITE".

**Ceilings, stated rather than left to be inferred.** The gate matches the SHAPE, not the
former helper NAME, so renaming it back buys no pass — but it does not reach a negative
check written some third way, and it does nothing about the root cause `§ Root cause`
names: `2>/dev/null` plus `|| RESULT=""` still discards the refutation for every other
assertion. Those remain § Resume option (2), still needing a live server.

**SHA:** `107bbd23045b473441647a1e4fa8519048b898a0`
**patch-id:** `77cb386d12ff05d1fa20bb9b9e85d422b604dc43`
## Tests added

`tests/mcp_smoke_scripts_reference_real_tools.rs` —
`no_smoke_script_asserts_absence_over_a_blankable_result`, a static gate needing no
server, with a non-vacuity assertion (>100 lines scanned) so a mistyped path cannot
report clean by finding nothing. Both of its branches were observed RED/green as tabulated
in § Fix.

No test was added for `call()`'s own fallback — that still needs the live server § Fix
describes, and is deliberately left to § Resume option (2).
## Workarounds

Do not use `assert_not_contains` in these scripts. If a negative check is genuinely
needed, assert first that `RESULT` is non-empty — the discriminator is already in the
variable, unused.

## Resume

Option (1) is **done** — the helper is deleted from both scripts and the shape is now
statically refused.

Option (2) remains open and still needs a live server: drop `2>/dev/null` from `call()`
and make a non-zero `mcp call` print the error and `exit 1` rather than blanking `RESULT`.
That is the root fix — it restores the refutation every OTHER assertion in these scripts
is currently missing, not just the deleted one. Demand an observed RED by pointing the
alias at a dead server and confirming the script reds instead of passing.
## References

- `tests/mcp-smoke-rust.sh`, `tests/mcp-smoke-kotlin.sh`
- `docs/trackers/issue-clusters/IC-16-assertion-that-cannot-fail.md`
