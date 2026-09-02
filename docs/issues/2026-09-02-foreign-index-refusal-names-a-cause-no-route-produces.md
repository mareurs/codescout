---
id: '43377426520f683b'
kind: bug
status: fixed
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
unverified: fixed and mutation-verified on branch foreign-index-route-column (25f6540c), NOT on experiments — both scripts are live-shimmed for every session in the shared checkout, so landing it is gated on a quiet tree rather than on any remaining work. Not archivable until then.
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

> **Amended 2026-09-02 — there is a FOURTH route, and this file asserting "three" was the
> same defect one level up.** `post-index-change-stage-log.sh:166-168` documents it in its
> own comment: `patch_paths()` reads **default-prefix headers only**, so a patch written
> with `--no-prefix` or applied with `-p0` "emits nothing and the write degrades to `-`".
> That is a distinct route from the blanket form — argv *does* name the patch file, and the
> paths inside it are unreadable rather than unnamed.
>
> Weaker evidence than the three above, and labelled as such: read from the code's comment
> at the bytes, **not** dated by `git log -S` and **not** observed at runtime. It is enough
> to settle the fix, which is the only thing it is used for.
>
> This is why § *Fix* takes level 2. A hand-maintained enumeration of routes was already
> short by one **on the day it was proposed**, by a reader who had the recorder open — so
> "list the real causes carefully" is not a remedy available to anyone, and level 1 would
> re-ship the frozen-prose defect with a fresh timestamp.

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

**Level 2 chosen, and the reproduction is what decided it.** Implemented, tested,
mutation-verified and **committed on a feature branch** — `foreign-index-route-column`,
SHA `689ceffb`, patch-id `b2be19e0bf0cb3aa6cc342e1b64035e2b21f0805`. **Not on
`experiments`**, so this file is NOT archivable yet; see § *Resume* for the reason and the
landing step.

Record the patch-id rather than relying on the SHA, and this file has now paid for the
rule rather than merely citing it. The commit was first made as `25f6540c`; rebasing onto
a moved `experiments` (`b6a7330d` -> `3151201a`, fifteen peer commits) orphaned it, and the
fix is now `689ceffb`. **The patch-id did not move** — `b2be19e0bf0cb3aa6cc342e1b64035e2b21f0805`
is byte-identical before and after, because it hashes the diff rather than its position.
So the SHA in this sentence has already been wrong once, in under an hour, exactly as
`CLAUDE.md` predicts; the patch-id beside it is what stayed resolvable.

Level 1 (replace the asserted cause with an enumeration of the real routes) is **not a
step toward** level 2, and it is worse than the file originally judged: the enumeration it
prescribed was **already short by one route on the day it was written**. This file's own
table names three; `post-index-change-stage-log.sh:166-168` documents a fourth in its own
comment — a patch applied with `--no-prefix` or `-p0` emits no paths from `patch_paths()`
and "the write degrades to `-`". So a hand-maintained enumeration reproduces the exact
defect being fixed: prose asserting a route set that the code has since outgrown. Level 2
cannot go stale that way, because the recorder writes what it did rather than a reader
predicting what it might have.

**The change.**

- `scripts/post-index-change-stage-log.sh` (+38/−10) — a fourth `route` column. Set at
  four sites: `named` (argv named this path), `unnamed` (staging op with a real id that did
  not name it — blanket form or non-default-prefix patch), `id-unset` (no
  `CLAUDE_CODE_SESSION_ID`), `not-staging` (unreadable or unrecognised parent). A row
  carried over from an existing log keeps its recorded route, and a three-column row is
  labelled `pre-route` rather than being given a guessed branch.
- `scripts/pre-commit-foreign-index.sh` (+52/−7) — collects the routes seen on `-` rows
  (`foreign_owners` dedupes to one `-` entry however many paths there are, so the branch is
  lost with the duplicates unless collected separately) and prints one line per distinct
  cause. The blanket-add line says outright that it is **probably the reader's own
  staging**, which is what the old sentence got backwards.

**STRICTLY DIAGNOSTIC, and this is the design constraint, not a note.** The refusal still
keys on the `owner` column alone. A mis-recorded route can therefore produce only a wrong
*explanation* — never a wrong refusal, and never a capture. That is what makes the change
safe to ship on a checkout where six sessions commit through the hook: the failure
direction is unchanged. The moment a route value gates the refusal, a recorder bug becomes
a correctness bug on a shared index; mutation M7 below exists to fail if anyone does that.

**The refusal and the remedy are untouched**, as this file required. Both were correct.

**Landed on a branch rather than on `experiments`, deliberately.**
`.git/hooks/post-index-change` is a thin shim exec'ing
`$root/scripts/post-index-change-stage-log.sh` off `--show-toplevel`, and
`.pre-commit-config.yaml:80` wires the guard by `entry:`. Writing either file **in the
shared checkout** changes commit-path behaviour for every session there on its next `git
add`, with nothing telling them it changed; six peer sessions held unstaged work at the
time.

A worktree isolates the tracked `scripts/` copy and the per-worktree stage log, so the
commit exercised the new recorder **on itself** — all four staged paths recorded `named`,
and the modified guard passed its own commit. **That isolation holds only because this diff
does not touch the shim:** `git rev-parse --git-path hooks` resolves to the **common**
`.git/hooks` from inside a worktree (measured by `codescout-0a` from
`.worktrees/tool-collapse`, confirmed here; `core.hooksPath` unset). A change that DID
touch the shim would go live for every session wherever it landed.
## Tests added

14 cases in `tests/hooks-discrimination.sh` § 8 (+117 lines), which runs in **CI**
(`.github/workflows/ci.yml:82`), not in pre-commit — so the tests could land separately
without touching a peer's commit path, but **must not**: they assert the route column, so
they would red CI until the scripts ship with them. The two move together.

One case per route-writing site, per CLAUDE.md § *Testing Discipline* — a route is written
at four sites and a kill at one says nothing about the other three.

**Results.** Baseline (unmodified scripts, original suite) 49/49. Modified scripts against
the **original** suite 49/49 — so no pre-existing case changes behaviour. Modified scripts
with the new cases **63/63**.

**Seven mutations, all killed**, each naming its own case:

| mutation | killed |
|---|---|
| M1 `route="named"` → wrong value | `explicit-path add records route=named` |
| M2 `${claim_route:-unnamed}` → `:-named` | `directory add records route=unnamed`, `guard names the blanket form` |
| M3 drop the `id-unset` branch | `add with no session id records route=id-unset` (+2) |
| M4 drop `claim_route="not-staging"` | `index write from an unrecognised parent records route=not-staging` |
| M5 drop the `pre-route` fallback | `a legacy three-column row is labelled pre-route, not blank` (+3) |
| M6 reinstate the false sentence | `guard no longer asserts the pre-guard cause` |
| M7 let a route value gate the refusal | `an unrecognised route value still refuses on the owner` |

M5's three extra kills are an artifact of the mutation, not of the change — the
modified-scripts-against-original-suite run above is what rules that out.

**The guard-text cases are a PAIR on purpose.** `has("blanket add")` is monotone under
widening; `hasnt("staged before this guard was installed")` is monotone under deleting the
whole block. Neither fires in the other's direction, so only together do they pin the
replacement. M6 is the mutation that proves it.

**Three harness bugs found while writing these, each returning a plausible result rather
than an error** — recorded because the next person will hit them:

1. A first draft of the `id-unset` case reused a path an earlier `git add <dir>` had
   already recorded, so it exercised the **carry-over** branch and printed no error while
   testing something other than what it claimed.
2. A `pre-route` fixture built from `git hash-object` never matched: the log stores the
   **abbreviated** blob `git diff --cached --raw` emits. A full 40-char sha silently
   re-derives the row instead of carrying it — a passing test of nothing. `blob_of()` in
   the suite exists for this and says so on the line.
3. Two mutations reported `TARGET ABSENT` because the runner's `||` delimiter had no escape
   against targets containing shell `||`. "Target absent" must not be read as "nothing to
   verify" — both mutations killed their case once the delimiter was fixed.
## Workarounds

Commit by pathspec, which never reads the shared index:

```
git commit -F <msg> -- <your paths>
```

This is what the guard itself prints, and it is correct.

## Resume

**The fix is committed on `foreign-index-route-column` (`689ceffb`, patch-id
`b2be19e0bf0cb3aa6cc342e1b64035e2b21f0805`). The one remaining step is landing it on
`experiments`, which is a coordination problem, not an engineering one.**

To land it: rebase or cherry-pick the branch onto `experiments` **at a moment when peers do
not hold unstaged work in the shared checkout**, then re-run `bash
tests/hooks-discrimination.sh` (expect 63/63). Re-run at least M6 (reinstate the false
sentence) and M7 (let a route value gate the refusal) before landing — those two guard the
prose replacement and the diagnostic-only invariant respectively, and they are the two a
reworker is most likely to break without noticing.

**Why the timing matters and is not fussiness.** The moment this reaches `experiments`,
both scripts are live for every session in the shared checkout on its next `git add`. The
change is designed so the failure direction is unchanged (§ *Fix*, diagnostic-only), so the
risk is low rather than absent — but the population that absorbs it did not ask for it and
will not be told. Land it when the tree is quiet.

This file becomes archivable once that happens, per `CLAUDE.md`'s archive trigger (verified
on `experiments`, gate green, regression test in place — the last two are already true).

**`codescout-0a` declined to carry it and was right to.** Routing a 278-line git-hook
diagnostic through a 13-task tool-surface-collapse branch would have made the whole-branch
review incoherent, coupled this fix's fate to eleven unrelated tasks, and — decisively —
**deferred and hidden the harm rather than avoiding it**: the hook change would still land
for every session at merge, buried where nobody reviews it as a hook change. They offered a
second pair of eyes on the diff instead, which is the useful form of help here.

**If a later session prefers the cheaper repair, read § *Fix* first.** Level 1 is not a
smaller version of this fix; it is the same defect re-shipped.
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
