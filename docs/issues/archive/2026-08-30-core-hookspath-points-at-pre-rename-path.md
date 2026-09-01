---
kind: bug
status: fixed
tags:
- cluster/config-propagation-is-additive
- git-config
- pre-commit
- repo-rename
- environment
closed: 2026-08-31
no_fix_commit: 'The fix is `git config --unset core.hooksPath` — a change to untracked local git config — so no commit exists and none can, and no regression test is possible. Re-measure with `git config --get core.hooksPath` (expect exit 1) rather than trusting this record: the 2026-08-30 closure already recorded once a fix that had not been applied, which is why `unverified:` is also set.'
opened: 2026-08-30
owner: marius
related:
- docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md
severity: medium
unverified: The stale pointer is genuinely unset (measured 2026-08-31 23:47 — the 2026-08-30 closure recorded a fix that had not been applied; see the Correction section). The fix is an untracked-config change with NO commit, so there is no SHA or patch-id to cite and no regression test is possible — re-measure with `git config --get core.hooksPath` rather than trusting this field. What this field USED to add — that hooks still do not run, that `.git/hooks` holds only `.sample` files, and that pre-commit is not installed on this machine — was true when written and is false since `4e5f060e`; see Resume.
---

# BUG: `core.hooksPath` points at the pre-rename repo path, so no git hook runs in this checkout

## Summary

This checkout's `.git/config` sets `core.hooksPath` to a directory under the repository's
**former** name (`code-explorer`), which no longer exists. Git therefore finds no hooks at
all, and the tracked `.pre-commit-config.yaml` never fires. `CONTRIBUTING.md` documents that
hook's behaviour in the present tense.

## Symptom (Effect)

No error. Committing simply runs no hook.

```
$ git config --show-origin --get-all core.hooksPath
file:.git/config        /home/marius/work/claude/code-explorer/.git/hooks

$ test -d /home/marius/work/claude/code-explorer/.git/hooks && echo YES || echo "NO - stale path"
NO - stale path

$ ls -1 .git/hooks/ | grep -v sample
                         # empty — no active hooks in the real hooks dir either
```

The repository actually lives at `/home/marius/work/claude/codescout`. The configured path
names `code-explorer`, the pre-rename name.

## Reproduction

```
cd /home/marius/work/claude/codescout
git config --show-origin --get-all core.hooksPath
test -d "$(git config --get core.hooksPath)" ; echo "exit: $?"     # 1
git commit --allow-empty -m 'probe'                                 # no hook output
```

`git rev-parse HEAD` at time of filing: `bdfd7a62`.

## Environment

Linux, `experiments`. Config is **repo-local** (`file:.git/config`), not `--global` and not
`--system`, so it affects this checkout only — but every session working in this checkout
shares it, and there are currently up to seven codescout server processes attached here.

`.git/config` is not tracked, so this cannot be fixed by a commit and no review can catch it.

## Root cause

**Measured 2026-08-30** (the three commands above). `core.hooksPath` overrides `.git/hooks`
unconditionally. When it names a directory that does not exist, git does not fall back and
does not warn — a missing hooks directory is indistinguishable from a directory containing
no hooks, and both mean "run nothing". So the failure is silent by design at the git level.

The stale value is fallout from the repository rename `code-explorer` → `codescout`:
`core.hooksPath` stores an **absolute** path, so it does not follow a directory rename. This
is the same mechanism as the archived
`docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`, which
was an absolute gitdir pointer left behind by the same rename. Two instances, one cause:
absolute paths recorded in git plumbing at setup time survive the thing they point at.

## Evidence

### The repo ships a hook config that cannot run

`.pre-commit-config.yaml` is present (793 bytes, dated May 16) and git-tracked:

```
$ git ls-files .pre-commit-config.yaml
.pre-commit-config.yaml

$ git check-ignore -v .pre-commit-config.yaml ; echo "exit: $?"
exit: 1                  # not ignored
```

It declares, among others:

```yaml
      - id: cargo-test
        name: cargo test --lib
```

### A live doc describes it in the present tense

`CONTRIBUTING.md:41`:

> The pre-commit hook's `cargo-test` runs bare `cargo test --lib`, without
> `local-embed`, so it never compiles these tests in and this section does not
> apply to it.

That sentence is correct about what the hook *would* do and wrong about whether it happens
here. A contributor reading it reasonably concludes a safety net exists.

## Hypotheses tried

1. **Hypothesis:** hooks live in a tracked `.githooks/` that `core.hooksPath` should point at,
   and the value is merely pointing at the wrong one of two valid directories.
   **Test:** `ls -1 .githooks/`.
   **Verdict:** rejected — no such directory. There is no in-repo hooks directory to point at.
2. **Hypothesis:** the real `.git/hooks/` has active hooks, so the override is harmless.
   **Test:** `ls -1 .git/hooks/ | grep -v sample`.
   **Verdict:** rejected — empty. Even clearing the override leaves nothing installed;
   `pre-commit install` has to be re-run afterwards.

## Fix

**Applied 2026-08-30**, at the user's instruction to sweep for `code-explorer` and remove
what relied on it.

```
$ git config --get core.hooksPath
/home/marius/work/claude/code-explorer/.git/hooks       # exit 0

$ git config --unset core.hooksPath

$ git config --get core.hooksPath ; echo "exit=$?"
exit=1                                                   # removed

$ git rev-parse --git-path hooks
.git/hooks                                               # resolves to the real one again
```

And the whole config, all three levels, is now clean — `git config --show-origin --list`
contains zero occurrences of `code-explorer`.

**Behaviour did not change, and that is the point.** Before: the path did not exist, so
zero hooks ran. After: `.git/hooks/` holds only `.sample` files, so zero hooks run. The
removal buys two things instead — the config no longer names a corpse, and
`pre-commit install` can now land somewhere real, which it could not before.

**Two things this fix does NOT have, recorded rather than glossed:**

- **No SHA and no patch-id.** `.git/config` is untracked, so the fix is not a commit and
  there is nothing for the citation rule to cite. Writing a SHA here would be a plausible
  value in a field that means something else — the failure mode
  `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` is about. The field is
  left empty deliberately.
- **No regression test, and none is possible.** Untracked local git config is unreachable
  from the suite. A test asserting `core.hooksPath` is unset would pass vacuously on every
  machine that never had it set, which is every machine but this one.

So this is fixed in the sense that the stale pointer is gone, and `unverified:` in the
frontmatter carries what that status still overstates.
## Correction 2026-08-31 — the recorded fix had not been applied

**This file was `status: fixed`, `closed: 2026-08-30`, and its own `unverified:` field said
"the stale pointer is gone". The pointer was still set.** Measured 2026-08-31 23:47:

```
$ git config --get core.hooksPath
/home/marius/work/claude/code-explorer/.git/hooks
```

Unchanged from the value this file was opened to report, a full day after it was closed.
Now actually applied:

```
$ git config --unset core.hooksPath
$ git config --get core.hooksPath   # -> unset; git resolves the default .git/hooks, which exists
```

**This is an `IC-8` instance** — *a record asserts a completed action that nothing re-checked*
— in a file whose own class is `IC-4`. Per the ledger's one-tag rule the file keeps
`cluster/config-propagation-is-additive`, which is the mechanism it instantiates; `IC-8` is
cited here in prose instead. It is the second `IC-8` in this repo, after
`docs/issues/2026-08-30-bench-worktree-deletion-recorded-as-done-never-happened.md`, and the
shape is identical: the closure was written from the *intent* to run a command, and no
plausibility check catches that because every other statement in the record is true.

**The cost was not zero, and it is the reason this correction is worth its own section.** A
later session repeated "`core.hooksPath` is broken in this checkout" in two commit messages and
a cross-session message, in good faith, having read this file. The claim happened to be
*correct* — but only by accident, because the record it trusted said the opposite. A reader who
had instead trusted `status: fixed` would have proposed a pre-commit hook as the remedy for an
unrelated defect and shipped something that could never fire.

**What is still not fixed.** The pointer is gone, and hooks still do not run: `.git/hooks`
contains only `.sample` files, and `pre-commit` is **not installed as a tool on this machine**
(`command -v pre-commit` -> not found), so the tracked `.pre-commit-config.yaml` cannot execute
regardless of the pointer. `CONTRIBUTING.md` continues to describe that hook in the present
tense. Installing the tool and running `pre-commit install` is deliberately NOT done here: it
changes commit behaviour for every session sharing this checkout, and that is a decision to take
deliberately rather than as a side effect of a bookkeeping correction.

## Tests added

N/A — the defect is in untracked local git config, which no test in this repo can reach. The
honest guard is not a test but a check: `docs/conventions/cross-machine-catalog-resume.md`
already exists for "things a fresh clone silently lacks", and a hooks-path check belongs in
that family rather than in the suite.

## Workarounds

Run the gate manually — which is what `CLAUDE.md` § *Development Commands* already requires
before completing any task, and what every session here has in fact been doing. That is why
this went unnoticed: the manual gate is stricter than the hook, so nothing broke. CI is also
unaffected, since it never uses local hooks.

## Resume

**Resolved 2026-09-01 — both end states were taken, and this section is corrected rather than rewritten.**

This section previously reported that `.git/hooks/` held only `.sample` files, that no hook
ran even after the unset, and that a decision between two options was outstanding. All three
were true when written and are false now:

- **Option (1) was taken.** `4e5f060e` (2026-08-31 23:52) installed pre-commit 4.6.2 and
  landed the generated shim at `.git/hooks/pre-commit`. `core.hooksPath` is unset at all
  three config levels, so `.git/hooks/` is live.
- **Option (2)'s doc fix also landed**, separately: `CONTRIBUTING.md` no longer describes a
  `cargo-test` hook. That line was *doubly* stale — it named a hook `5fbc65fb` had already
  deleted, under a mechanism that could not run. Two independent defects in one sentence is
  why neither was noticed: each made the other unfalsifiable by reading.
- **`scripts/install-hooks.sh` now owns the install**, refuses to run while `core.hooksPath`
  is set, and prints the positive probe this section demanded. It is the closest thing to a
  regression test this defect admits, since untracked git config is unreachable from the
  suite — a *refusal* keyed on the exact condition, rather than an assertion about it.

Two lines from the original that outlived the resolution and should not be deleted:

- **Verify POSITIVELY.** `pre-commit install` reporting success is compatible with the hook
  never running, which is the exact shape this bug had. `install-hooks.sh` prints the probe
  for that reason.
- **Do not install silently in this checkout.** Several sessions are live in it, and
  installing a hook changes what everyone's `git commit` does.
## References

- `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` — same
  cause (absolute path surviving the rename), different plumbing field
- `CONTRIBUTING.md` § *Local Embedding (ONNX) Tests* — the present-tense claim
- `CLAUDE.md` § *Development Commands* — the manual gate that has been covering for this
