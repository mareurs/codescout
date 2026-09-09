---
kind: bug
status: open
tags:
- cluster/unclassified
closed: ''
opened: 2026-09-09
owner: marius
related: []
severity: low
---

# BUG: `install-hooks.sh`'s seeded-pair count prints `0\n0`, because `grep -c` reports its answer on stdout and a *different* fact through its exit status

## Summary

`scripts/install-hooks.sh`'s stage-log seeding line reads
`seeded="$(grep -c . "$seed_log" 2>/dev/null || echo 0)"`. When the seeded log is empty,
`grep -c` prints `0` **and exits 1** — "matched nothing", not "failed" — so the `|| echo 0`
fallback fires on a perfectly good result and appends a second zero. `$seeded` becomes the
two-line string `0\n0`, and the install summary breaks across two lines.

Cosmetic only: the value is correct whenever the count is non-zero, because `grep -c` exits
0 then. It is filed because the mechanism is not cosmetic — a fallback keyed on an exit
status that answers a different question than the one the caller is asking.

## Symptom (Effect)

From a fresh install where the index holds no staged pairs, verbatim from stdout:

```
ok      stage log             seeded, 0
0 inherited pair(s) marked unknown
```

Intended: `ok      stage log             seeded, 0 inherited pair(s) marked unknown`.

## Reproduction

Tree `5e49fd13` (`experiments`). Observed 2026-09-09 08:0xZ while reproducing bug
`7f03effb66d8da39` in a throwaway repo — the seeding branch runs whenever
`$git_dir/session-stage-log` does not already exist.

The mechanism in isolation:

```
$ cd "$(mktemp -d)" && : > empty.txt
$ s="$(grep -c . empty.txt 2>/dev/null || echo 0)"; printf 'seeded=[%s]\n' "$s"
seeded=[0
0]
$ grep -c . empty.txt; echo "rc=$?"
0
rc=1
```

## Environment

Linux, `experiments`. Platform-independent — POSIX `grep` specifies exit 1 for "no lines
selected", and `-c` does not change that.

## Root cause

**`grep -c` uses stdout for the count and exit status for a match/no-match verdict; the
caller reads the exit status as a success/failure verdict and composes a fallback on it.**

- `scripts/install-hooks.sh` — the `seeded=` assignment in the stage-log seeding block
  (search `inherited pair(s) marked unknown`).

`||` fires on exit 1, but exit 1 here does not mean the command failed — it means the count
was zero, which is exactly the case the fallback was written to supply. Both branches
produce output and both land in the same command substitution.

Measured 2026-09-09 by the two-command transcript above; not inferred from the source.

## Evidence

### This site was scanned, opened, and mis-cleared four hours before it was found

At ~06:2xZ on 2026-09-09, sessionId `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30` swept
`scripts/**` and `tests/*.sh` for `2>/dev/null` + `||` fallbacks — 31 sites, 14 files —
while filing `04aa6207d31a861f`. This line came up as a candidate, they opened it, and they
published it as **correct**:

> `grep -c` exits **1** on an empty file while printing `0`, so the fallback normalizes an
> exit status into the number it already means […] Counted in the 27.

**The first clause is right and the conclusion is the opposite of what follows from it** —
`||` does not *replace* stdout, it *appends*. The note therefore carried, in its own text,
the premise sufficient to falsify its own verdict, one step away. It was retracted in place
after this file was opened independently; their table's count moved from 27 correct to 26.

**The transferable half is about the clearing, not the mechanism.** § *Testing Discipline*
instructs that a re-derivation which confirms be published, because a confirmation is a
**denominator** rather than a catch. That instruction needs a caveat it does not carry: **a
confirmation that is WRONG is not a denominator, it is a foreclosure.** A finding invites the
next reader to check it; a clearing tells them not to bother. Under-counting the 31-site sweep
would have cost nothing — mis-clearing one site cost an independent rediscovery of a defect a
peer had already stood in front of.

So the law wants its derivation published alongside the verdict, not instead of it: here the
derivation *was* published and was sufficient to catch the error, and nobody checked it,
because the sentence ended in "correct". Raised by the author of the mis-clearing, against
themselves.

### The rendered output

The full install-time line, captured during the worktree reproduction for
`7f03effb66d8da39`, is quoted verbatim under § *Symptom*. `$seeded` is interpolated into a
single `echo`, so the line break is inside the variable rather than in the format string.

## Hypotheses tried

1. **Hypothesis** — the second `0` comes from a second seeding call.
   **Test** — read the block; it is one `echo` with one `$seeded` interpolation, guarded by
   `elif [ -e "$seed_log" ]`.
   **Verdict** — rejected. One call, one variable, two lines inside the value.

## Fix

Applied 2026-09-09 at `scripts/install-hooks.sh:397` — exactly the shape this file prescribed,
including its refusal of `|| true`:

```sh
seeded="$(grep -c . "$seed_log" 2>/dev/null)"
seeded="${seeded:-0}"
```

**One fact measured while applying it, load-bearing enough to sit in the code comment:** this
form is only safe because `install-hooks.sh` sets `-uo pipefail` and deliberately **not** `-e`.
Under `set -e` an assignment from a command substitution exiting 1 aborts the script, so an
empty seed log would become a *failed install* rather than a wrapped line — a strictly worse
defect than the one being fixed. Verified by control:

```
$ bash -c 'set -e; : > /tmp/e.txt; v="$(grep -c . /tmp/e.txt 2>/dev/null)"; echo survived'
$ echo $?
1        # "survived" never printed
```

The test file already recorded the same `-e` fact independently at
`tests/pre-push-foreign-session-guard.sh:696`.

**Not yet committed** — no `fix_sha` / `fix_patch_id` yet, so this file stays `open`. Archive
is gated on the fix being on `experiments`.
## Tests added

Added to `tests/pre-push-foreign-session-guard.sh` — section *"the seeded stage-log count is one
number, not two"*, four assertions, placed against the existing `installer_fixture` helper as this
file prescribed.

**Two-sided, plus a control**, because the prescribed assertion alone is not enough:

| assertion | guards against |
|---|---|
| exactly one summary line | the seeding branch never running (the monotone direction) |
| no line begins with a bare wrapped count | the wrap itself |
| renders a single `0` | the wrap itself, positively |
| non-empty index renders `2` (**control**) | `seeded` being hard-wired to `0` |

**Observed red, by mutating the production path** — restoring `|| echo 0` in
`install-hooks.sh` and re-running: **88 passed, 2 failed**, the two failures being *"no line
begins with a bare wrapped count"* and *"renders a single 0"*. Fix restored: **90 passed, 0
failed**.

Recorded because it qualifies the suite honestly: *"exactly one summary line"* stayed **green**
under the armed mutation. The wrapped output still contains exactly one line matching
`inherited pair(s) marked unknown`, so that assertion does not discriminate this defect — it is
the paired positive guarding the monotone direction, and nothing more. The control also stayed
green, confirming it is not merely tracking the same signal as the two that fired.
## Workarounds

None needed — the reported count is correct, it just wraps. A reader parsing the install
log for `seeded, N` should tolerate `N` being followed by a newline.

## Resume

Apply the two-line form above to the `seeded=` assignment in `scripts/install-hooks.sh`,
and add the `^[0-9]+ inherited` absence assertion to the installer section of
`tests/pre-push-foreign-session-guard.sh`. Confirm the RED by restoring `|| echo 0`.

## References

- `docs/issues/archive/2026-09-09-install-hooks-writes-to-git-dir-so-a-worktree-install-reports-ok-and-does-nothing.md`
  — found while reproducing that bug; the same script, a different mechanism.
- `tests/pre-push-foreign-session-guard.sh` — the installer sections, which already build a
  fixture that reaches this branch.
