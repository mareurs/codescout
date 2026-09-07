---
id: ac66453338e3507c
kind: bug
status: fixed
title: 'BUG: an unstaged `.pre-commit-config.yaml` blocks every session''s commits, and the holder is the one party that cannot see it'
tags:
- cluster/shared-resource-carries-no-owner
closed: 2026-09-07
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

**FIXED 2026-09-07 by removing the reader — `074b749e`, patch-id
`4c3958557408b19cdf60354a5f8288167e4342e4`.**

The pre-commit framework is uninstalled. The commit stage now runs
`scripts/pre-commit-run.sh` through a direct shim, so **nothing reads
`.pre-commit-config.yaml` at commit time** and no state of that file — dirty, staged,
deleted — can refuse anything. `_has_unstaged_config` is not reached because the entry point
containing it is not invoked. This is elimination rather than mitigation: the analysis below
remains correct about the framework and is now correct about a component this repo no longer
runs.

**Point 2 below is falsified and deliberately left standing.** It says
`scripts/install-hooks.sh`'s native `pre-commit` shim *"DOES NOT EXIST — the repo is designed
not to have it"*. True when written; the design changed. `install-hooks.sh` now removes a
framework-generated shim before `install_shim` runs, which is the one ordering that makes the
migration safe — a framework shim whose config is absent exits 1 on **every** commit in the
clone. Kept rather than edited because a reader who finds the old claim and the new file
learns something a corrected sentence would hide: the obstacle was a design decision, and
design decisions are the kind of obstacle that moves.

**THE RESIDUAL, STATED SO IT IS NOT MISTAKEN FOR ZERO.** `pre-commit install` succeeds
against any config — measured, including one with no `repos:` key and `repos: []` — and would
reinstall the framework over the native shim, restoring this bug exactly. Nothing in the
config can prevent that; the retired file's header says so and `install-hooks.sh --check`
reports which shim is live. The normal install path no longer invokes `pre-commit install` at
all, so this needs someone to type it deliberately.

### The first production observation, and I caused it

This file records that its reproduction was only ever run in a throwaway repo, *"never on the
shared checkout — doing so CAUSES the outage this file describes, which is why nobody had run
it"*. On 2026-09-07 I ran it by accident, on the shared checkout, while building the fix.
Editing `.pre-commit-config.yaml` at 09:50 made it dirty with the framework shim still live;
every commit in the clone was blocked until the native shim landed at 09:52.

Two things that only a live instance could show:

- **The window opens at the FIRST KEYSTROKE, not at the deletion.** I had measured the
  deletion case in a throwaway repo beforehand and ordered the swap around it. The framework
  refuses on an unstaged *modification* too — same outcome, earlier trigger — so my guard was
  aimed at the second half of a window that had already opened.
- **The blocked path's own suggested remedy is a capture.** The framework prints
  `git add .pre-commit-config.yaml` to fix this`. Following it would stage the *holder's*
  in-flight config edit into the blocked party's commit, under their message and their
  `Session-Id` trailer — precisely what `pre-commit-unreviewed-content.sh` and
  `pre-commit-foreign-index.sh` exist to refuse. A guard whose refusal text routes the reader
  into a different guard's failure.

Reported by sessionId `cda3afe5-17b8-4863-9f4c-9fe4eadbc17b`, who hit it on an ordinary
pathspec commit, held rather than working around it, and declined the suggested remedy for
that reason. The summary's own claim held exactly — the holder is the one party who cannot
see it, and I was the holder.

<details><summary>Superseded analysis, correct for the framework this repo no longer runs</summary>

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

### Measured 2026-09-07 — the placement is solved and the ATTRIBUTION is not

The bullet above is **half wrong, and it is the expensive half.** Reproduced end-to-end in a
throwaway repo (never on the shared checkout — doing so *causes* the outage this file
describes, which is why nobody had run it):

**1. No hook can pre-empt the refusal, including a `local:` one.** Verified in pre-commit
4.6.2: `_has_unstaged_config` runs inside the framework's own entry point, gated on
`stash = not args.all_files and not args.files`, and returns before dispatching anything.
The probe carried a canary hook as its control, which is what makes the absence a
measurement rather than a broken fixture:

| config state | canary hook | commit |
|---|---|---|
| clean | **ran** | rc=0 |
| dirty, unstaged | **did not run** | rc=1, names the file |

**2. The alternative location this file proposed DOES NOT EXIST.** § *Resume* previously
sent the next session to `scripts/install-hooks.sh`'s "native `pre-commit` shim". There is
none: that script delegates the whole pre-commit stage to `pre-commit install`, and its
`install_shim` explicitly **REFUSES** to overwrite a framework-generated hook. A session
following the old Resume would have gone looking for a file the repo is designed not to
have.

**3. A WRAPPER works, and that is the buildable path.** Moving the framework's shim aside
and installing a native `pre-commit` that runs first and then `exec`s it: the wrapper's
output appeared *above* the framework's refusal, with the refusal still correctly firing.
So "a check that fires before pre-commit's own refusal" is reachable — the obstacle was
never ordering.

**4. And it still cannot name the holder — which RECLASSIFIES this bug.** The wrapper in
the probe printed a holder line, and the value it produced was the last *committer* of the
file, not the party holding the current unstaged edit. That is attribution by proximity,
which `IC-17` and `OB-8` both forbid; the wrapper would ship a confident wrong name.

The reason is structural rather than a missing lookup: the resource here is the **unstaged
working tree**, which is `IC-17`'s own `NONE` row — *"Git has no per-path unstaged ownership
concept, so this is the one gap with no adjacent primitive to extend."*
`session-stage-log` covers **staged** pairs only, so the machinery cited in the bullet above
is real and out of reach here by one step.

**So this is not an independent item with an unbuilt remedy; it is a CONSEQUENCE of the
working-tree gap, and it cannot be closed ahead of it.** What a wrapper can honestly deliver
is **scope, not ownership**: *"this refusal is global — it blocks every session on this
checkout, and the holder is unrecorded."* That is a smaller claim than the § *Fix* bullet
promised and it is the one that is true. It still converts the outage from a confusing
per-session error into a legible checkout-wide condition, which is most of the value; it
does not convert an unbounded wait into a message, which was the stated goal.

*(Derived, not cited: run the probe again rather than trusting this table — it is ten lines
of `bash` in a `mktemp -d` repo and takes one call. The version is load-bearing; the gate
condition on `stash` could move.)*

</details>
## Tests added

None, and the gap is worth naming rather than excusing. A regression test would have to
dirty the shared config and attempt a commit, which reproduces the outage it tests for. A
test of the *owner field* would be ordinary once that exists.

## Workarounds

`git add .pre-commit-config.yaml` — but only the holder should do it; staging another
session's file files their write under your name. A blocked peer's correct move is to ask
the holder to land it, and `--no-verify` is the wrong habit here specifically.

## Resume

**The placement question is CLOSED (measured 2026-09-07, below). What is open is a
decision, and it is a narrower one than this file previously posed:** ship a wrapper that
announces the *scope* of the outage, or ship nothing until the working-tree gap is closed.
Do not go looking for a way to name the holder — that route was measured shut.

Next concrete step if the answer is "ship the wrapper": amend `scripts/install-hooks.sh`,
whose `install_shim` currently **refuses** this shape by design, and decide whether the
refusal should gain an exception or the wrapper should be installed by a separate path.
That refusal is correct as written — it exists to catch someone running `pre-commit install
--hook-type` over a native hook — so widening it is a deliberate change, not a bug fix.
## References

- `OB-10` in `docs/trackers/observer-blindness.md` — the class, its membership test, and
  the enumeration of other files with this shape.
- `docs/issues/2026-09-01-pre-commit-stash-removes-every-peers-unstaged-work.md` — the
  other shared-state defect in the same tool, found in the same pass.
- `.pre-commit-config.yaml` header — the whole-tree-diff defect that withdrew the pre-push
  hooks, and the `--no-verify`-teaching argument this bug feeds.
- `9e493b20` — the commit whose editing window produced this; it also shortened two hook
  runtimes for a related reason.
