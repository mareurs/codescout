---
id: '6d62c19ed88c4a06'
kind: bug
status: open
title: install-hooks.sh writes to --git-dir, so a worktree install reports ok and installs nothing
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
topic: git hooks and shared-checkout tooling
closed: ''
opened: 2026-09-09
severity: high
---

# BUG: `install-hooks.sh` writes hooks to `--git-dir`, so from a worktree it installs nothing and reports `ok`

## Summary

`scripts/install-hooks.sh` computes its destination from `git rev-parse --git-dir`
(`:100`, `:224`). In a **linked worktree** that is `.git/worktrees/<name>`, which has no
`hooks/` directory — while git actually reads hooks from `git rev-parse --git-path hooks`,
i.e. the **common** dir. The write fails, and because the script runs `set -uo pipefail`
with no `-e`, the unconditional `echo "ok … shim installed"` at `:271` reports success
anyway and the script exits 0.

Two sessions are working in `.worktrees/` on this checkout right now, so it is reachable
today rather than theoretically.

## Symptom (Effect)

From inside a linked worktree, `bash scripts/install-hooks.sh` prints (stderr interleaved):

```
scripts/install-hooks.sh: line 174: /…/.git/worktrees/<name>/hooks/pre-commit: No such file or directory
chmod: cannot access '/…/.git/worktrees/<name>/hooks/pre-commit': No such file or directory
ok      pre-commit      shim installed -> scripts/pre-commit-run.sh
```

`ok` on stdout, exit `0`, and **no file written anywhere**. Not an inert hook — no hook.

`--check` from the same cwd reports every hook `MISSING` and exits 1. Its printed remedy is
*"run without --check"*, which prints `ok` and installs nothing, so `--check` reports
`MISSING` again. **That is a loop with no exit**, and each half is individually plausible.

## Reproduction

Tree: `d5104ff3` (`experiments`). Verified 2026-09-09 06:1x Z without running the installer —
the three facts that compose the defect were each read directly:

```
$ git rev-parse --git-dir                     # main checkout
.git
$ git rev-parse --git-path hooks              # where git READS
.git/hooks

$ git -C .worktrees/tool-collapse rev-parse --git-dir
/home/marius/work/claude/codescout/.git/worktrees/tool-collapse
$ git -C .worktrees/tool-collapse rev-parse --git-path hooks
/home/marius/work/claude/codescout/.git/hooks          <- git reads HERE
$ test -d /…/.git/worktrees/tool-collapse/hooks && echo YES || echo NO
NO                                                     <- installer writes HERE
```

`grep -n mkdir scripts/install-hooks.sh` → **0 matches**, so nothing creates the directory.

## Environment

Linux, `experiments`, shared checkout with live linked worktrees under `.worktrees/`.
Platform-independent — nothing here depends on Linux.

## Root cause

**`--git-dir` and `--git-path hooks` diverge in a linked worktree, and the script uses the
first to write what the second is read from.**

- `scripts/install-hooks.sh:100` — `git_dir="$(git rev-parse --git-dir 2>/dev/null)"`
- `scripts/install-hooks.sh:224` — `dest="$git_dir/hooks/$hook_name"`
- `scripts/install-hooks.sh:269-271` — `render_shim … "$dest"; chmod +x "$dest"; echo "ok …"`

The `ok` is **unconditional**: it is not guarded by the success of either preceding command,
and `set -uo pipefail` (`:58`) omits `-e`, so two failed commands do not stop the function or
set `fail`. The script therefore exits 0.

Measured 2026-09-09 by reading the three `rev-parse` values above and grepping for `mkdir`;
the installer was **not** run from a worktree, because two peer sessions are live in those
trees and a write there is theirs to authorise, not mine.

**Two properties make this worse than an ordinary path bug:**

- **The success line is a claim nothing re-checks.** `install_shim`'s `--check` arm
  (`:242-266`) byte-compares the installed shim against `render_shim`'s output — a genuinely
  good verification — and the **install** arm has no equivalent. The one path that writes is
  the one path that does not verify.
- **The two halves disagree and each is locally correct.** `--check` says `MISSING` because
  the file really is missing *from the path it looks at*; install says `ok` because it never
  looks. Neither is lying; the shared premise is wrong.

## Evidence

### The suite cannot catch this by construction

`tests/pre-push-foreign-session-guard.sh:358-441` regenerates the shim by copying the real
`scripts/install-hooks.sh` into a throwaway directory and asserting on the result. The
throwaway is a plain `git init` repo — **never a linked worktree** — so `--git-dir` and
`--git-path hooks` coincide there and the divergence cannot arise. The section's assertions
are correct and cannot reach this.

### It composes with the shim's degrade-open design

`.git/hooks/pre-push` degrades open by design when its target is missing
(`scripts/install-hooks.sh:213-217`), which is right. But a worktree session has **no shim at
all**, so there is nothing to degrade: `pre-commit`, `post-index-change` and `pre-push` simply
never run. On a shared checkout that includes the foreign-index guard and the session-stage
log, i.e. exactly the machinery that keeps concurrent sessions from capturing each other's
work.

## Hypotheses tried

1. **Hypothesis** — the installer writes an inert file into the worktree's gitdir that git
   never reads (the shape first proposed to me).
   **Test** — `grep -n mkdir scripts/install-hooks.sh`; read `:169-219` and `:269-271`.
   **Verdict** — rejected, and the correction matters. There is no `mkdir`, so `cat >` fails
   and **nothing is written**. "No hook, reported as installed" is a different defect from
   "a hook git ignores", and only the second would be found by looking for a stray file.

2. **Hypothesis** — `--check` masks it.
   **Test** — read the `--check` arm at `:242-266`.
   **Verdict** — rejected, and the truth is worse. `--check` reports `MISSING` and exits 1;
   its remedy produces `ok`; the loop closes with no state changed.

## Fix

Not applied. Two changes, and the second is the one that generalises:

- **Use the hooks path git actually reads.** `git rev-parse --git-path hooks` returns the
  common dir's `hooks/` from a worktree and `.git/hooks` from the main checkout, so it is
  correct in both and needs no worktree special-casing. `git_dir` is also used at `:295-329`
  for `session-stage-log` seeding, which is a *different* question — per-worktree state is
  arguably right there — so change the hooks destination only, deliberately, rather than
  swapping `git_dir` wholesale.
- **Make the success line conditional.** `echo "ok … shim installed"` must not run when
  `render_shim` or `chmod` failed. Given no `set -e`, this needs explicit `|| { fail=1;
  return; }` on both. An unconditional success line is the defect underneath the path bug:
  fix only the path and the next write failure reports `ok` just as loudly.

## Tests added

None yet. The regression test must add a **linked worktree** fixture to
`tests/pre-push-foreign-session-guard.sh`'s installer section — `git worktree add` in the
throwaway, run the installer from inside it, and assert the hook exists at
`--git-path hooks`. Its acceptance criterion is an observed RED: with the `--git-dir` form
restored, that assertion must fail.

Second assertion, independent of the path: force `render_shim` to fail (unwritable
destination) and assert the run does **not** print `ok` and does **not** exit 0.

## Workarounds

Run `bash scripts/install-hooks.sh` from the **main checkout**, never from inside
`.worktrees/<name>`. Hooks are shared through the common dir, so one correct install covers
every worktree. To verify from anywhere:

```
ls -l "$(git rev-parse --git-path hooks)"/pre-push
```

## Resume

Change `dest` at `scripts/install-hooks.sh:224` to derive from `git rev-parse --git-path
hooks` rather than from `git_dir`, and gate the `ok` at `:271` on both preceding commands
succeeding. Then add the linked-worktree fixture described in § *Tests added* and confirm the
observed RED by reverting the path change.

Do **not** run the installer from inside `.worktrees/tool-collapse` or
`.worktrees/result-cap-marker-gate` while verifying — both held live sessions as of
2026-09-09 06:12:12Z.

## References

- `scripts/install-hooks.sh:58` (`set -uo pipefail`, no `-e`), `:100`, `:224`, `:242-266`,
  `:269-271`.
- `tests/pre-push-foreign-session-guard.sh:358-441` — the installer section whose throwaway
  is never a linked worktree.
- `docs/conventions/cross-machine-catalog-resume.md` — the sibling class of "a clone arrives
  missing a layer, and each layer is silent in a different way".
