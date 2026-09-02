---
id: '43377426520f683b'
kind: bug
status: open
title: 'BUG: the foreign-index guard''s refusal names a cause no route produces, and omits the route it exists to catch'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
- hooks
- shared-checkout
- diagnostics
- pre-commit
topic: shared-checkout commit guards
closed: ''
opened: 2026-09-02
owner: marius
related:
- docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md
severity: medium
---

## Summary

`scripts/pre-commit-foreign-index.sh` refuses a bare commit whose index holds a path
owned by `-`, and its `Staged by:` block explains that `-` as *"staged before this guard
was installed"*. Three routes in the paired recorder produce `-`, and that is none of
them. The route it omits — a **blanket-form `git add`** (`-A`, `-u`, `.`, a directory) —
is both the commonest and the exact case the guard was built to catch, so a session doing
precisely the thing the guard exists for is told it is looking at pre-guard residue.

The refusal is correct, the printed remedy is correct and works first try. Only the
explanation is wrong.

## Symptom (Effect)

`scripts/pre-commit-foreign-index.sh:209-212`, verbatim:

```
      (unrecorded) — staged before this guard was installed, so no
          session claimed it. Unknown is deliberate: it over-refuses
          until the pair churns out, where claiming it would have gone
          silent. Find the owner by asking, not by assuming it is yours.
```

Observed 2026-09-02 by a peer session in this checkout, which staged
`git add .codescout/audit/` — a directory — 40 seconds before committing, in its own
session, with `CLAUDE_CODE_SESSION_ID` set to a real value. `git diff --cached
--name-only` returned exactly that one path. `git commit -F <msg>` refused, listing that
path under `theirs:` and printing the sentence above.

The session's own reading: *"A session reading that text learns the wrong thing about
its own tree — and specifically learns 'this is not mine' about something that is."*

## Reproduction

At `HEAD`, in a checkout with the hooks installed and `CLAUDE_CODE_SESSION_ID` set:

1. `git add <a directory>` — any blanket form: `-A`, `-u`, `.`, or a directory path.
2. `git commit -F <msg>` with no pathspec.
3. Refused; the `Staged by:` block reads `(unrecorded) — staged before this guard was
   installed`, which is false — it was staged seconds ago, by you, after the guard
   existed.

Contrast, same session, same route: `git add -- <explicit paths>` records the real owner
and the identical bare commit passes. That asymmetry is the tell.

## Environment

Linux, codescout `experiments`, shared checkout, pre-commit framework. Independent of
session count — concurrency only raises the odds someone meets it.

## Root cause

**Prose frozen while the code it describes grew two new branches.** Three routes write
`-`:

| route | site | condition |
|---|---|---|
| 1 | `post-index-change-stage-log.sh:72-76`, `:88` | no `CLAUDE_CODE_SESSION_ID` — `me="${CLAUDE_CODE_SESSION_ID:--}"` |
| 2 | `:88-92` | *"an unreadable or unrecognised parent yields 'not staging', which downgrades to `-`"* |
| 3 | `:158`, `:250-255` | **blanket form** — `names_path()` compares each STAGED path against the literal argv tokens; a directory token never matches a file beneath it, so `owner="-"` at `:290` |

The guard's sentence names a fourth thing that no branch produces.

*Measured 2026-09-02*, by `git log -S` on each string:

```
e3c75306  2026-09-01 02:01:38   the cause sentence lands
fa9b3aff  2026-09-01 03:58:14   route 3 lands (names_path, "Blanket forms degrade")
92dfa4e4  2026-09-01 14:00:58   the `git apply --cached` patch-path route lands
```

The sentence predates both later routes by two and twelve hours. It was plausibly true
when written — with only route 1 in existence, an unrecorded pair really did suggest
pre-guard staging — and the code then gained routes to the same state without anything
reading the guard's prose. There is no authoring error to find, which is what makes this
`IC-11` rather than a mistake.

**And the guard cannot compute the true route at refuse time.** The log's record is
`owner \t blob \t path` (`:293`) — the *reason* for `-` is discarded at write time and is
not recoverable at read time. So the guard is not failing to look something up; the
information does not exist on its side of the boundary.

## Evidence

### The two sessions' contrasting outcomes, same route

Both staged via the same mechanism in their own sessions with a real
`CLAUDE_CODE_SESSION_ID`:

- peer, `git add .codescout/audit/` (directory) → `names_path` miss → `-` → refused.
- this session, `git add -- <17 explicit paths>` → every staged path literally in argv →
  real owner recorded → identical bare commit passed all four hooks.

That rules out route 1 for both and isolates route 3 as the discriminator.

### The recorder documents route 3 as correct and intended

`post-index-change-stage-log.sh:250-255`:

> Deliberately NOT matched: a directory prefix. `git add src/` stages every file beneath
> src/ INCLUDING a peer's, so treating the subtree as named would be exactly the false hit
> this function exists to avoid. **Blanket forms degrade to `-` and that is correct** — a
> blanket add followed by a BARE commit is the capture this guard exists for, so making it
> loud is the point.

So the behaviour is right and only the explanation is wrong — which is why this file
proposes no change to the refusal.

## Hypotheses tried

1. **Hypothesis:** the recorder failed to capture a `git add` issued through codescout's
   `run_command`, so the path was never recorded.
   **Test:** read `.git/session-stage-log`.
   **Verdict:** rejected — the path IS present; what is `-` is the *owner*, not the entry.
   Proposed by the reporting session and withdrawn by it.

2. **Hypothesis:** `CLAUDE_CODE_SESSION_ID` was absent in the reporting session (route 1).
   **Test:** that session printed it.
   **Verdict:** rejected — set, non-empty, real.

3. **Hypothesis:** this session's own 17 staged paths were never recorded, so the guard's
   absent→`mine` branch silently claimed them, inverting its stated over-refusal.
   **Test:** inspect the log for this session's id.
   **Verdict:** **rejected, and the reasoning was unsound** — `:294-297` rebuilds the log
   from `git diff --cached` on every index write, so it is a snapshot of the index, not a
   history. The paths had left the index at commit. The snapshot cannot answer a question
   about the past, and the absent→`mine` asymmetry is separately documented as an intended
   fail-open at `:57` and `:60-70`. Recorded because it was nearly filed as a second
   finding.

## Fix

Not implemented. Two levels.

**Cheap, and sufficient for the reported harm.** Replace the single asserted cause with
the three real routes, leading with the blanket form since it is the common case and the
one the guard exists to catch. The over-refusal keeps all its value; only the sentence
stops asserting.

**Better, and it makes the true cause knowable.** Record the route beside the owner —
a fourth column at `post-index-change-stage-log.sh:293` — so the guard can name the
branch that actually fired instead of enumerating candidates. This is the difference
between a diagnostic that lists what *might* have happened and one that reports what
*did*, and the cost is one field.

Do **not** change the refusal or the remedy. Both are correct; `:254-255` documents the
pathspec form as the unaffected path and it worked first try in the observed case.

## Tests added

None — capture-on-notice. A regression test would assert the `Staged by:` block names no
cause absent from the recorder's branch set, which is only cheap once the route is
recorded (fix level 2); against the level-1 fix it degrades to a string match.

## Workarounds

Commit by pathspec, which never reads the shared index:

```
git commit -F <msg> -- <your paths>
```

This is what the guard itself prints, and it is correct.

## Resume

Decide between fix level 1 and level 2 before writing either — level 2 subsumes level 1
and makes the regression test worth having, so shipping level 1 first would be work
thrown away rather than a step toward it.

## References

- `scripts/pre-commit-foreign-index.sh:209-212` — the sentence.
- `scripts/post-index-change-stage-log.sh:72-76`, `:88-92`, `:158`, `:250-255`, `:290-297`
  — the three routes, the log record shape, and the snapshot rebuild.
- `docs/issues/archive/2026-09-01-git-apply-cached-stages-but-records-no-owner.md` —
  cited at `:162-164`; the same surface, where a route recording `-` made the guard refuse
  the stager's own commit. That one was a real defect in the recorder; this one is the
  explanation only.
- Found by a peer session in this checkout on 2026-09-02, which observed the refusal and
  identified the sentence as an inference stated as fact. The route that fired was
  established afterwards by reading `names_path()`.

