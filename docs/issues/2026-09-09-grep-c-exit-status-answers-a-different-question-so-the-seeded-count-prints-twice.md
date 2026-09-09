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

The full install-time line, captured during the worktree reproduction for
`7f03effb66d8da39`, is quoted verbatim under § *Symptom*. `$seeded` is interpolated into a
single `echo`, so the line break is inside the variable rather than in the format string.

## Hypotheses tried

1. **Hypothesis** — the second `0` comes from a second seeding call.
   **Test** — read the block; it is one `echo` with one `$seeded` interpolation, guarded by
   `elif [ -e "$seed_log" ]`.
   **Verdict** — rejected. One call, one variable, two lines inside the value.

## Fix

Not applied. Drop the fallback and let `grep -c` speak for itself, since it already prints
`0` in the no-match case:

```sh
seeded="$(grep -c . "$seed_log" 2>/dev/null)"
seeded="${seeded:-0}"
```

The second line still covers the genuinely-absent-file case, where `grep` prints nothing.
Do **not** answer this with `|| true` — that keeps the composition and only hides the
status; the defect is reading the status at all.

## Tests added

None. This should be covered by the installer section of
`tests/pre-push-foreign-session-guard.sh`, whose fixture already reaches the seeding branch
with an empty index: assert the install log contains no line matching `^[0-9]+ inherited`,
which reds exactly on the wrap.

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
