---
kind: bug
status: fixed
tags:
- cluster/config-propagation-is-additive
- git-config
- pre-commit
- repo-rename
- environment
closed: 2026-08-30
opened: 2026-08-30
owner: marius
related:
- docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md
severity: medium
unverified: 'Hooks still do not run — the stale pointer is gone but no hook is installed; `pre-commit install` has not been run. Also: the fix is an untracked-config change with NO commit, so there is no SHA or patch-id to cite, and no regression test is possible.'
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

The stale pointer is removed; what remains is a **decision**, not an investigation.

`.git/hooks/` holds only `.sample` files, so no hook runs even now. Two coherent end states,
and nobody has picked one:

1. Run `pre-commit install` and let the tracked `.pre-commit-config.yaml` fire. Verify
   POSITIVELY — `git commit --allow-empty` and observe hook output — because
   `pre-commit install` reporting success is compatible with the hook never running, which
   is the exact shape this bug had.
2. Correct `CONTRIBUTING.md:41`, which describes the `cargo-test` hook in the present
   tense, and leave hooks uninstalled. Defensible: the hook (`cargo test --lib`) is strictly
   weaker than the four-command gate `CLAUDE.md` already mandates, which is why nothing
   broke and nobody noticed.

Do not do (1) silently in this checkout — several sessions are live in it, and installing a
hook changes what everyone's `git commit` does.
## References

- `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` — same
  cause (absolute path surviving the rename), different plumbing field
- `CONTRIBUTING.md` § *Local Embedding (ONNX) Tests* — the present-tense claim
- `CLAUDE.md` § *Development Commands* — the manual gate that has been covering for this
