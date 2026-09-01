---
id: c629f1ddb35bf2e9
kind: bug
status: open
title: 'BUG: an unstaged `.pre-commit-config.yaml` blocks every session''s commits, and the holder is the one party that cannot see it'
tags:
- cluster/shared-resource-carries-no-owner
---

# BUG: an unstaged `.pre-commit-config.yaml` blocks every session's commits, and the holder is the one party that cannot see it

## Summary

While `.pre-commit-config.yaml` has **unstaged** changes, `pre-commit` refuses to run at
all — for every session on the checkout, on every commit, before any hook executes. On a
six-session checkout that is a global commit outage produced by one session's ordinary
in-progress edit. The editor is the single party who never observes it, because the act of
finishing their work (staging the file) is what clears the condition.

## Symptom (Effect)

Reported by `codescout-3c` at 21:19 on 2026-09-01, while this session held the file dirty:

```
Your pre-commit configuration is unstaged.
`git add .pre-commit-config.yaml` to fix this.
```

Exit 1, emitted **before** the first hook line. The message is accurate and names the
file. It does not name **who** holds it dirty, or whether the holder is still active, so
the blocked party's only options are to wait an unbounded time, guess who to ask, or
`--no-verify`.

## Reproduction

On any checkout with `pre-commit install`:

```bash
printf '\n# touch\n' >> .pre-commit-config.yaml   # modify, do NOT stage
git commit -m 'anything' -- some/other/file.md   # from ANY session, including this one
# -> Your pre-commit configuration is unstaged.
```

**Do not reproduce this on a shared checkout.** It blocks every concurrent session for as
long as the file stays dirty, which is why the mechanism below was confirmed from
`pre-commit`'s source rather than by re-running it.

## Environment

pre-commit 4.6.2 (pipx), git, Linux, `codescout` on `experiments`, six live sessions in
this checkout across three `CLAUDE_CONFIG_DIR` profiles.

## Root cause

`pre_commit/commands/run.py:353` — before any hook is dispatched:

```python
if stash and _has_unstaged_config(config_file):
    logger.error(
        f'Your pre-commit configuration is unstaged.\n'
        f'`git add {config_file}` to fix this.',
    )
    return 1
```

and the condition, `run.py:330`:

```python
def _has_unstaged_config(config_file: str) -> bool:
    retcode, _, _ = cmd_output_b(
        'git', 'diff', '--quiet', '--no-ext-diff', config_file, check=False,
    )
    return retcode == 1
```

So the test is **working tree vs index on that one path**, and it is gated on `stash` —
the normal commit path, where pre-commit intends to stash unstaged work and therefore
refuses to run against a config it would have to stash out from under itself. That is
correct behaviour for a single-user checkout. Measured 2026-09-01: read from the installed
package at `~/.local/share/pipx/venvs/pre-commit/lib/python3.14/site-packages/pre_commit/`,
not inferred from docs.

**The observer-blindness is in the asymmetry, not in the check.** `git diff --quiet` is
cleared by *staging*, and staging is how the editor finishes. So the editor's own next
commit succeeds and the condition disappears in the same motion — no signal is ever
delivered to the party who can act on it, while every other session takes the full block.
This session held the file dirty for roughly eight minutes and learned of it only because
a blocked peer chose to send a message instead of working around it.

## Evidence

The peer, having hit it and stopped:

> Your unstaged .pre-commit-config.yaml is blocking commits for every session on this
> checkout right now — pre-commit refuses with "Your pre-commit configuration is unstaged"
> before running any hook. […] I will not `git add` your config, and I am not reaching for
> --no-verify, because the second one is precisely the habit your change exists to stop
> teaching.

Two details worth keeping. They declined the two available workarounds — staging another
session's file, and `--no-verify` — and both refusals were correct: the first files a write
under the wrong author, the second is the habit the very change being edited exists to stop
teaching. And the resolution required *guessing the owner*: nothing in the refusal points
at a session, so the peer inferred it from having just discussed the file with this one.

## Impact

Every session on the checkout, on every commit, for as long as the file is dirty. Severity
is bounded by duration and duration is controlled by the one party with no signal, which is
what makes it worse than its blast radius suggests. It also pushes directly toward
`--no-verify`: the blocked party's work is unrelated to the config, so the refusal reads as
noise, and `.pre-commit-config.yaml`'s own header documents that a noisy hook teaches
exactly that habit.

## Fix

**Not fixed. The mechanism is upstream and correct; what is missing is an owner field and a
practice.**

- **Practice (available now, zero code):** treat this file as *edit-and-land*, never
  *edit-and-hold*. Stage or commit it in the same working step you modify it. This is a
  property of the RESOURCE rather than of the edit, so it applies to every file with the
  same shape — see `OB-10` for the membership test and the current enumeration.
- **Owner field (the real remedy, unbuilt):** the refusal names the file and not the
  holder. This repo already has the machinery — `scripts/pre-commit-foreign-index.sh`
  resolves staged paths to a `Session-Id` and prints a `SendMessage(to: "uds:…")` address.
  A `pre-commit`-stage check that fires *before* pre-commit's own refusal and reports "held
  dirty by codescout-3e (pid 303936)" would convert an unbounded wait into a message. That
  is `IC-17`'s remedy exactly: add an owner to a shared resource, never a better listing.
- **Not a candidate:** telling people to be careful. The party who would have to remember
  is defined by the mechanism as the one receiving no signal.

## Tests added

None, and the gap is worth naming rather than excusing. A regression test would have to
dirty the shared config and attempt a commit, which reproduces the outage it tests for. A
test of the *owner field* would be ordinary once that exists.

## Workarounds

`git add .pre-commit-config.yaml` — but only the holder should do it; staging another
session's file files their write under your name. A blocked peer's correct move is to ask
the holder to land it, and `--no-verify` is the wrong habit here specifically.

## Resume

Decide whether the owner-field check is worth building: a `pre-commit`-stage hook that runs
before pre-commit's internal refusal, detects `git diff --quiet .pre-commit-config.yaml`
returning 1, and resolves the holder the way `scripts/pre-commit-foreign-index.sh` already
resolves stagers. Ordering is the open question — pre-commit's check at `run.py:353` runs
before any hook, so a hook cannot pre-empt it and the check may have to live in
`scripts/install-hooks.sh`'s native `pre-commit` shim instead.

## References

- `OB-10` in `docs/trackers/observer-blindness.md` — the class, its membership test, and
  the enumeration of other files with this shape.
- `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — the
  other shared-state defect in the same tool, found in the same pass.
- `.pre-commit-config.yaml` header — the whole-tree-diff defect that withdrew the pre-push
  hooks, and the `--no-verify`-teaching argument this bug feeds.
- `9e493b20` — the commit whose editing window produced this; it also shortened two hook
  runtimes for a related reason.

