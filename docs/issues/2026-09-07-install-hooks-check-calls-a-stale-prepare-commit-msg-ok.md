---
id: ceb1f7a11823d71f
kind: bug
status: open
title: 'BUG: `install-hooks.sh --check` reports `ok` for a STALE `prepare-commit-msg`, the one hook it checks by presence'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
topic: git hook installation verification
closed: ''
opened: 2026-09-07
owner: marius
related: []
severity: medium
---

# BUG: `install-hooks.sh --check` reports `ok` for a STALE `prepare-commit-msg`, the one hook it checks by presence

## Summary

`scripts/install-hooks.sh --check` byte-compares every shim against `render_shim()` — except
`prepare-commit-msg` on the no-`--with-session-id` path, which falls back to a **presence**
predicate. That is the exact defect the script fixed for its other hooks on 2026-09-06
(`grep -q "$target"` → `cmp -s`); the fix was not carried to this branch. A stale, trap-bearing
`prepare-commit-msg` is reported as `ok`.

## Symptom (Effect)

Same checkout, same moment, two invocations disagreeing:

```
$ scripts/install-hooks.sh --check
ok      prepare-commit-msg    shim present (opt-in, installed earlier)

$ scripts/install-hooks.sh --check --with-session-id
STALE   prepare-commit-msg      shim differs from what this script generates
```

The shim in question had no degrade-open clause:

```
$ grep -c 'DEGRADE OPEN' .git/hooks/prepare-commit-msg
0
```

i.e. it carried the `exec`-on-missing-target 127 trap that would refuse **every commit** in
this checkout if `scripts/prepare-commit-msg-session-id.sh` were absent — the failure the
2026-09-06 clause exists to prevent.

## Reproduction

```
git rev-parse HEAD          # 4b30601c at filing
```

1. Install the trailer hook, then let the shim's shape change in a later commit (or plant a
   pre-2026-09-06 shim at `.git/hooks/prepare-commit-msg`).
2. `scripts/install-hooks.sh --check` → `ok      prepare-commit-msg    shim present`, and the
   footer does not name it.
3. `scripts/install-hooks.sh --check --with-session-id` → `STALE`, exit 1.

Observed live 2026-09-07 on this checkout after a 119-commit fast-forward.

## Environment

Linux (`ripper`), bash, `experiments` @ `4b30601c`, pre-commit 4.x on PATH,
`core.hooksPath` unset.

## Root cause

`install_shim()` does the byte-comparison, and is only reached for `prepare-commit-msg` when
`with_session_id=1`. The `--check`-without-the-flag branch is a separate `elif` that tests
`-x "$git_dir/hooks/prepare-commit-msg"` and prints `ok … shim present (opt-in, installed
earlier)` — existence, never currency. Read at `scripts/install-hooks.sh` in the
`if [ "$with_session_id" = "1" ]; then … elif [ "$check_only" = "1" ]; then …` block near the
end of the script.

The branch is *correct about what it set out to say* — it exists so a `--check` run reports
what is on disk rather than what this invocation's flags would install, which is itself a
deliberate fix ("Reporting `skip` for a hook that is in fact live would be a status tool lying
about the status it exists to report"). The defect is that it answers **installed?** while the
word it prints, `ok`, is read as **installed and current?** — the same word the byte-compare
path prints when it means the stronger thing.

measured 2026-09-07: the two `--check` invocations above, run seconds apart on the same shim,
returning `ok` and `STALE`.

## Evidence

### The script's own header states the predicate it rejects

`scripts/install-hooks.sh`, above `render_shim()`:

> `--check` used to ask `grep -q "$target" "$dest"` — "does the shim MENTION the target path".
> Every shim this script has ever written mentions it, including the pre-2026-09-06 shape with
> no degrade-open clause, so the predicate was monotone under exactly the drift that matters.

Presence is monotone in the same way, for the same reason, on the branch the fix did not reach.

### The footer inherits the flaw

With only this hook stale and the flag absent, `fail` is never set, so the run exits `0` and
prints no STALE footer — the summary is silent about a hook that is running an older shape.

## Hypotheses tried

1. **Hypothesis:** the shim was current and `--with-session-id` re-renders spuriously.
   **Test:** `grep -c 'DEGRADE OPEN' .git/hooks/prepare-commit-msg`.
   **Verdict:** rejected — returned `0`; the installed shim genuinely predated the clause.

## Fix

Route the `--check`-without-flag branch through the same `render_shim()` + `cmp -s`
comparison the other hooks use, reporting `STALE` when it differs and keeping the existing
`off … opt-in; not installed` wording only for the genuinely-absent case. The comparison needs
no flag: whether the hook *should* be installed is a policy question, but whether the installed
bytes match the generator is not.

- **SHA (experiments):** pending
- **patch-id:** pending

## Tests added

None yet. The natural home is `tests/pre-push-foreign-session-guard.sh`, which already owns a
shim section written because "the suite's 34-assertion aggregate had zero of them on this
clause" — the same aggregate-hides-the-case shape.

## Workarounds

Always pass `--with-session-id` to `--check` on a machine where the trailer hook is installed;
it forces the byte-comparison. Note this is the opposite of intuition — the *narrower-looking*
invocation is the more thorough one.

## Resume

Edit `scripts/install-hooks.sh`: in the trailing `if [ "$with_session_id" = "1" ] … elif
[ "$check_only" = "1" ]` block, replace the `-x "$git_dir/hooks/prepare-commit-msg"` presence
test with a `render_shim` + `cmp -s` comparison, setting `stale=1` and `fail=1` on mismatch so
the STALE footer fires. Then re-run both invocations and confirm they agree.

## References

- `scripts/install-hooks.sh` — the generator, the byte-compare, and the branch that skips it
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md`
- `docs/issues/archive/2026-08-30-core-hookspath-points-at-pre-rename-path.md` — the prior
  silent-hook-wiring failure this script was written against

