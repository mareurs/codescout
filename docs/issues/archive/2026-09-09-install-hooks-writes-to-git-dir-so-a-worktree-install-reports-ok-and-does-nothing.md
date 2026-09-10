---
id: 7f03effb66d8da39
kind: bug
status: fixed
title: install-hooks.sh writes to --git-dir, so a worktree install reports ok and installs nothing
owners:
- marius
tags:
- cluster/record-asserts-an-unchecked-completion
topic: git hooks and shared-checkout tooling
closed: 2026-09-09
opened: 2026-09-09
severity: medium
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

**This subsection originally claimed that a worktree session has no shim at all, so
`pre-commit`, `post-index-change` and `pre-push` "simply never run". That is FALSE, and
measuring it is what moved this bug's severity from `high` to `medium`.**

git reads hooks from the **common dir** for a linked worktree exactly as it does for the
main checkout. Measured 2026-09-09 in a throwaway: hooks installed into
`$(git rev-parse --git-path hooks)` fired on a `git commit` **and** a `git push` issued from
inside the linked worktree —

```
PRE-COMMIT-FIRED cwd=/tmp/tmp.Xua2PPnWDL/wt
PRE-PUSH-FIRED  cwd=/tmp/tmp.Xua2PPnWDL/wt     <- push refused, hook exit 9
```

And on this checkout, all three live worktrees resolve to the same place:

```
.worktrees/doctor-per-project-isolation -> /home/marius/work/claude/codescout/.git/hooks
.worktrees/result-cap-marker-gate       -> /home/marius/work/claude/codescout/.git/hooks
.worktrees/tool-collapse                -> /home/marius/work/claude/codescout/.git/hooks
```

So the worktree sessions were covered throughout, by the main checkout's install. **What is
real is narrower and still worth fixing:** a session that runs the installer *from* a
worktree is told it installed three hooks and installed none, and `--check` from there then
contradicts it. On a checkout where nobody had installed from the main tree, that is the
whole guard silently absent — the defect is the false report, not a standing outage.

The original claim was written from reading the three `rev-parse` values without running
anything, which is exactly the state § *Root cause* labels "a hypothesis wearing a
conclusion's clothes". It is preserved here rather than deleted because the severity it
justified is what put this bug at the front of the queue.

### Confirmed from inside a real worktree, and the hook did not merely resolve — it REFUSED

The correction above rests on a throwaway plus a path resolution. Stronger evidence arrived
the same evening from sessionId `b0015a98-e290-46de-8ed1-3c94bc73a987`, working in
`.worktrees/result-cap-marker-gate`, and it is better in the way that matters: a path that
*resolves* to the common dir is consistent with a hook that never fires, while a hook that
**refuses a push** cannot be.

That session's `pre-push` fired three times from inside the linked worktree — once printing
its own degrade-open warning, then **twice refusing** a push, naming `c95ba99b` on one and
six sids on the other. Reported independently of this file, from the worktree rather than
about it.

### The window check was right by luck: `%ad` is the wrong clock

§ *Fix* claims no commit landed in the ≤49s hooks-deletion window. The claim holds, but the
method used to establish it did not: it read `git log --format=%ad`, which is the **author**
date, to answer a question about when a commit was **made**. Those are different clocks and
they diverge in this very history —

```
68d72d67   a=09:34:49   c=10:33:14      <- 58 minutes apart, rebased
5e49fd13   a=10:52:24   c=10:52:24      <- the one that was read; coincided
```

Re-derived on committer date across all refs, which is the query that actually answers it:

```
$ git log --all --since='2026-09-09 11:06:00' --until='2026-09-09 11:06:49' | wc -l
0
```

And independently by `b0015a98-e290-46de-8ed1-3c94bc73a987` over `experiments`, also empty,
with their own two commits at `c=10:33:14` — 33 minutes before the window on committer time
and 90 on author time, which is exactly the gap that makes the wrong clock look convincing.
**A rebase rewrites committer date and preserves author date, so on any rebased branch
`%ad` under-reports recent activity** — the direction that makes a "nothing happened in this
window" claim come out false-clean. Use `--since`/`--until` (committer date) or `%cd`
explicitly whenever the question is *when did this land*, never *who wrote it when*.

**Which clock `--since`/`--until` uses was ASSERTED above and is now MEASURED.** The
re-derivation is only sound if the range filter is committer-based, and that premise was
stated rather than checked. Measured by sessionId `bf6a6925-f207-4a2f-8135-95e7563e859f`
and reproduced here, using `68d72d67` — whose clocks differ by 58 minutes — as the probe:

```
68d72d67   a=09:34:49   c=10:33:14
window around COMMITTER date (10:33:00–10:33:30)  ->  22 commits
window around AUTHOR    date (09:34:30–09:35:10)  ->   0 commits
```

So the range filter is committer-based and the re-derivation was sound **by construction,
not by luck** — the luck was confined to the original `%ad` read. The precise distinction is
**display versus filter**: `%ad` in a `--format` is author date *even when the range filter
beside it is committer-based*, which is how one query can look wrong and be right, or the
reverse. Naming the two halves separately is what keeps the correction from over-generalising
into "date handling here is unreliable".

**A rebase has a SECOND consequence, and it is the expensive one.** `%ad` under-reporting is
a measurement error a re-derivation fixes. The other is that a rebase **orphans any SHA a
peer cited before it ran** — the 22 commits above share committer date 10:33:13–14 with author
dates spanning 2026-09-03 to 09-09, which is the rebase's signature in the log. The same
session re-checked `dac1068a`, a SHA it had already cited in a handoff document at 09:15
+03:00, i.e. **before** that rebase. It survived only because it predates the rebase base —
and they would not have known without looking. The patch-id recorded alongside it
(`371bee7c5081481311866cd797d7591abb2c5bc3`) is the reason the citation was safe either way.
That is CLAIM → EVIDENCE for CLAUDE.md's "cite a fix by SHA *and* patch-id" rule, from a live
near-miss rather than from the 2026-08-19 census that produced the rule.
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

Applied on `experiments` at **`3f0d13b7`**, patch-id
**`74db845b1fa599d0f736ba6dd23bfbf26c813fbb`**.

**THE POPULATION WAS FIVE SITES, NOT THE TWO THIS SECTION ORIGINALLY NAMED.** Running the
reproduction before working this plan is what found the other three. Each fails in its own
direction, and a per-site fix following the plan as written would have shipped three more
instances of one mechanism behind a passing test:

| site | what it does | how it failed in a worktree |
|---|---|---|
| `install_shim`'s `dest=` | writes the shim | write fails, `ok` printed anyway |
| the `ok` line | reports the write | unconditional — see below |
| the framework-shim `-f` test | detects a `pre-commit`-generated shim | never fires, so such a shim in the common dir is unseen and gets clobbered by the very refusal written to prevent it |
| `rm -f "$git_dir/hooks/pre-commit"` | removes it | `rm -f` on a nonexistent path **exits 0**, so the branch printing "framework shim removed" removes nothing |
| `--check`'s `-x` probe for `prepare-commit-msg` | reports whether it is live | reports `off` for a hook that IS installed and running |

All five now resolve through one `hooks_dir`, computed once from `git rev-parse --git-path
hooks`.

**`git_dir` was deliberately NOT swapped wholesale.** The `session-stage-log` seed must stay
per-worktree, because **both** of its readers resolve it with `--git-dir`
(`scripts/post-index-change-stage-log.sh:84`, `scripts/pre-commit-foreign-index.sh:104`).
Seeding the common dir while both hooks read the worktree dir would break stage attribution
in every worktree, silently — a worse defect than the one being fixed, and invisible.

**No `mkdir -p`, deliberately.** It would create the missing directory and write a hook git
never reads — a stray file emits nothing at all, where a failed write emits an error — and
it would make the regression test vacuous, since that test reds precisely *because* the
wrong-path write fails.

**The success line is now conditional on the write**, with an explicit failure branch on both
`render_shim` and `chmod`. Ordering is load-bearing: `chmod` runs only if `render_shim`
returned 0, so a partially written shim can never be made executable, and git skips a
non-executable hook. The worst end state is "no hook, reported FAILED, exit 1".

**`hooks_dir` is absolutised at the source**, which the original plan did not call for.
`--git-path hooks` returns a *relative* `.git/hooks` from a main checkout and an *absolute*
path from a worktree, so the bare value means different things to different readers. That is
safe inside this script only because of its `cd "$PROJECT_ROOT"`, which is a property of the
file rather than of the value. Measured 2026-09-09 while verifying this very fix: a
verification script took the relative form and ran `rm -f "$hooks/pre-push"` from another
directory, **deleting three live hooks out of the real checkout**. Restored and re-verified
(`--check` exit 0, five `ok` lines); no commit landed in the ≤49s window, and the index held
zero staged pairs, so no stage attribution was lost.
## Tests added

`tests/pre-push-foreign-session-guard.sh`, two new sections (86 assertions total, 0 failed):

- **`== install-hooks.sh writes into the hooks dir git READS, from a linked worktree ==`** —
  builds a real `git worktree add` fixture, runs the installer from inside it, and asserts
  all three shims land at `--git-path hooks`. The linked worktree is the load-bearing
  detail: every other installer section in that file uses a plain `git init` repo where
  `--git-dir` and `--git-path hooks` **coincide**, so no assertion added there can ever
  reach this. The section therefore asserts its own discriminating property first — that the
  two really do diverge — so a `worktree add` that silently did not happen reds instead of
  passing vacuously.
- **`== a failed write does not print ok, and does not exit 0 ==`** — removes write
  permission rather than moving the path, so it stays meaningful even if the hooks dir is
  resolved some third way later. Skips loudly (as a FAIL, not a silent pass) under uid 0,
  where mode bits cannot express the case.

Also pinned: **every `ok … shim installed` line has a shim behind it**, count and equality
together — equality alone is satisfied by `0 == 0`. That is the assertion the defect was
actually about.

**Two guarded sites, two mutations, two distinct kills — neither fails under the other's:**

| mutation of the production script | worktree section | write-failure section |
|---|---|---|
| `dest` restored to `"$git_dir/hooks/$hook_name"` | **6 FAIL** | 3 PASS |
| `ok` line made unconditional again | 8 PASS | **3 FAIL** |

The second reads `3 ok-line(s) with nothing written` and `want exit 1, got 0` — this bug's
signature verbatim. Both mutations were taken against a **copy** of `scripts/` and `tests/`
rather than in place: this is a shared checkout, and the suite copies `$INSTALLER` into a
throwaway either way, so the mutation reaches the same bytes without arming a broken
installer for peers.

CI already runs the suite in its own non-root job (`push-guard-tests`,
`.github/workflows/ci.yml`), so the unwritable-dir case executes there rather than skipping.
## Workarounds

Run `bash scripts/install-hooks.sh` from the **main checkout**, never from inside
`.worktrees/<name>`. Hooks are shared through the common dir, so one correct install covers
every worktree. To verify from anywhere:

```
ls -l "$(git rev-parse --git-path hooks)"/pre-push
```

## Resume

N/A — fixed at `3f0d13b7` (patch-id `74db845b1fa599d0f736ba6dd23bfbf26c813fbb`), gate green,
regression test with an observed RED at each of the two guarded sites.

One incidental defect found in the same script while reproducing this one is filed
separately and is still open:
`docs/issues/2026-09-09-grep-c-exit-status-answers-a-different-question-so-the-seeded-count-prints-twice.md`.
## References

- `scripts/install-hooks.sh:58` (`set -uo pipefail`, no `-e`), `:100`, `:224`, `:242-266`,
  `:269-271`.
- `tests/pre-push-foreign-session-guard.sh:358-441` — the installer section whose throwaway
  is never a linked worktree.
- `docs/conventions/cross-machine-catalog-resume.md` — the sibling class of "a clone arrives
  missing a layer, and each layer is silent in a different way".
