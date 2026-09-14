---
kind: bug
status: fixed
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-07
opened: 2026-09-07
owner: marius
related:
- docs/trackers/observer-blindness.md
severity: high
---

# BUG: the pre-push guard's refusal text EXECUTES its own example commands — an unquoted heredoc turns a documented `git push` into a recursive one, and the refusal never prints

## Summary

`scripts/pre-push-foreign-session-guard.sh:147` opens its refusal banner with `cat >&2 <<EOF`
— **unquoted**. Line 224 contains markdown-style backticks around a documented command:

```
  were answering, and `git push origin $branch` would have satisfied the instruction
```

Bash performs command substitution inside an unquoted heredoc, so those backticks are not
formatting — they are a **command**. Expanding the banner runs `git push origin experiments`,
which fires `pre-push` again, which expands the same banner, which runs the push again.

Two consequences, and the second is worse than the resource cost:

1. **Unbounded recursion.** Each level forks a `git push` and an `ssh` to GitHub. Measured
   2026-09-07: **45 processes** across guard/push/ssh from a single `git push`, still growing
   when killed.
2. **The refusal NEVER PRINTS.** `cat` blocks during expansion, so the banner reaches nobody.
   A guard whose entire product is a question that must be asked *at the one moment it can be
   answered* asks nothing. It presents as a silent hang.

## Symptom (Effect)

`git push origin experiments` hangs indefinitely with **no output at all** — no refusal, no
error, no progress. `origin` never moves. The process tree grows:

```
git push ─┬─ ssh
          └─ guard.sh ─ guard.sh ─ git push ─┬─ ssh
                                             └─ guard.sh ─ guard.sh ─ git push ─ …
```

The operator sees a slow push and reasonably concludes "network".

## Reproduction

Deterministic, on any checkout whose push carries a foreign `Session-Id` commit:

```
local_sha=$(git rev-parse HEAD); remote_sha=$(git rev-parse origin/experiments)
CLAUDE_CODE_SESSION_ID=<your-sid> timeout 15 bash -x \
  ./scripts/pre-push-foreign-session-guard.sh origin <remote-url> \
  <<< "refs/heads/experiments $local_sha refs/heads/experiments $remote_sha"
```

Exits 124 (timeout). The trace's last two lines are the whole bug:

```
+ cat
++ git push origin experiments
```

`++` is command substitution — the banner is executing, not printing.

## Environment

`experiments` at `c7db63eb`. Shared checkout, 5 codescout sessions live at 14:47:38Z.

## Root cause

An **unquoted heredoc delimiter is load-bearing and correct here** — the banner interpolates
`$foreign_report`, `$me`, `$branch` and `$foreign_sids`, so `<<'EOF'` would break the message.
That makes backticks in the body live syntax with **no escape the author reached for**.

Introduced at `41377049` (2026-09-07T10:38:19+03:00), *"fix(hooks): an authorisation names a
SET, a branch push sends a PREFIX"* — a commit that **improved the refusal prose**. It is
already on `origin/experiments`, so every clone has it.

This is `IC-6` / `cluster/addressing-without-an-escape-hatch`, and specifically the **heredoc
tell** CLAUDE.md already names: *"a construct that exists precisely to mean 'this is data, not
syntax' will be misread by every scanner in the process."* The four prior instances were
scanners misreading a heredoc body as command text. This one is **bash itself**, which is the
same class with the highest possible authority — the misreader is the interpreter.

Note what makes the line *look* safe: every other command in the banner is written as an
indented block, which needs no backticks. Line 224 is the only one written **inline in prose**,
where markdown habit supplies backticks. The defect is not carelessness about shell quoting; it
is a **markdown reflex inside a shell string**, and it will recur on the next inline mention.

## Evidence

- `grep -n 'cat >&2 <<' scripts/pre-push-foreign-session-guard.sh` → `147:cat >&2 <<EOF`
- Backticks in the heredoc body (lines 148–294): **exactly one line, 224**.
- `git log -L 224,224:...` → `41377049`.
- `git merge-base --is-ancestor 41377049 origin/experiments` → **yes**.

## Hypotheses tried

Two wrong readings, both discarded against the bytes, and the sequence is the lesson:

1. *"The guard recursively calls `git push`."* Read off the process tree. **Refuted** by the
   source: the only `git push` strings are inside the heredoc — which I then dismissed as inert
   text. That dismissal was the error; the heredoc is unquoted, so it is not inert.
2. *"Git is blocked on an index lock held by a peer."* **Refuted** — `rev-parse`, `status`,
   `log` all return instantly.

The process tree was right and the reasoning about it was wrong, in both directions in turn.
Only `bash -x` with the trace captured **to a file** settled it: the same trace piped to `tail`
returned nothing, so the first two attempts to read it produced no evidence at all.

## Fix

**Fixed 2026-09-07 on `experiments` — `9dd4792f`, patch-id
`29dfd7ca487e46c7780df8de465652e11fa2a359`.** Escape the backticks so bash passes them through
as text, keeping `$branch` expanded as every other command in the banner does:

```
  were answering, and \`git push origin $branch\` would have satisfied the instruction
```

**Escaping the `$` as well was tried first and is wrong** — it prints the literal string
`$branch` where the rest of the banner prints the branch name, so the one line describing a
hazard about a specific branch would be the only one not naming it.

**A quoted delimiter (`<<'EOF'`) is NOT the fix** and should not be reached for later: the
banner interpolates `$foreign_report`, `$me`, `$branch` and `$foreign_sids`, so quoting it
would replace one broken message with four.

**Verified on the real repo, not only in the harness** — the harness structurally cannot
reproduce this (see *Tests added*). Against `experiments` at `c7db63eb` with 15 foreign
commits in range: exit 1 in **0 seconds** (previously a 25s timeout), **0** push processes
spawned, banner present, and the offending line renders as
`` `git push origin experiments` `` — backticks intact, `$branch` expanded.

## Tests added

Three, in `tests/pre-push-foreign-session-guard.sh`. **The suite was 69 passed / 0 failed in
1.35 seconds with the fork bomb live**, and the reason is the interesting part.

**Why the existing 69 could not see it.** The harness builds a throwaway repo with **no
`origin` remote**, so the injected `git push origin main` failed instantly and its stderr
vanished into the substitution. The bug needs a *reachable* remote to recurse, and the harness
structurally cannot have one — `cluster/repro-env-diverges-from-gate-env`.

**So the obvious test does not work, and this is the load-bearing finding.** Asserting the
banner is *present* PASSES under the bug: a failed substitution still lets `cat` finish.
Measured — `has "and the banner actually printed"` was **green on the RED run**. What
discriminates is asserting the example survives as **literal text**, since under the bug it is
replaced by the substitution's (empty) output.

1. `inline example survived as literal text` — site-specific; observed RED before the fix.
2. `heredoc body has no live backtick` — class-level, scans the unquoted heredoc body for any
   backtick not backslash-escaped. Caught line 224 by name on the RED run. This is what sees
   the *next* inline example someone adds.
3. `and the scanner reached a real body (N lines)` — **non-vacuity control for (2)**, without
   which (2) is worthless: an emptiness assertion is monotone under removal, so renaming or
   reflowing the `cat >&2 <<EOF` opener would make the awk match nothing and pass while
   scanning air. Pins the body at ≥100 lines (it is 114).

Also: `run()` now wraps the guard in `timeout 20`. Without it a hang **hangs the suite**
rather than redding it — no assertions report and no exit code is produced, which is strictly
worse than a failure because CI shows an unfinished job instead of a failed test.

**What is still not covered:** no test drives a refusal against a repo with a *reachable*
remote, so the recursion itself is still only reproducible by hand. Assertion (2) makes that
acceptable — it forbids the precondition rather than detecting the effect.

**And assertion (2) is scoped to THIS script's heredoc, not to the population.** Swept
2026-09-07: `scripts/` holds **5** unquoted heredocs — `pre-push-foreign-session-guard.sh`
(fixed), `post-index-change-stage-log.sh` (×2), `retrieval-stack.sh`, `install-lsp.sh` — and
**zero** of the other four currently contain a live backtick, so the repo is clean today. Two
of them are git hooks, so the same prose edit there costs the same. The repo-wide lint is
deliberately **not** written here: its natural home is `tests/hook_config.rs`, which already
makes cross-script assertions, and re-implementing heredoc parsing in a second language to
assert about the first is the shape CLAUDE.md warns is indistinguishable from coverage. Left as
a named follow-up with its population already measured, rather than as a worse test.

## Workarounds

`git push --no-verify` bypasses the hook — **and bypasses the authorisation question the guard
exists to ask**, so it is not a workaround, it is the failure. Until fixed, publish by refspec
one rung at a time (the ladder the banner itself describes) only after asking the operator, or
apply the one-line fix first.

## References

- `scripts/pre-push-foreign-session-guard.sh:147` (opener), `:224` (the backticks)
- `tests/pre-push-foreign-session-guard.sh:295` (the monotone assertion)
- `41377049` — introducing commit
- CLAUDE.md § *Parsers Over a Namespace* — the heredoc tell
- CLAUDE.md § *Testing Discipline* — monotone assertions; the guard's own predicate-vs-remedy ceiling
- `docs/trackers/observer-blindness.md` `OB-20` — why the guard exists

## Fix provenance

- **SHA:** `9dd4792f` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `29dfd7ca487e46c7780df8de465652e11fa2a359` — content hash of the diff; survives rebase and cherry-pick.
