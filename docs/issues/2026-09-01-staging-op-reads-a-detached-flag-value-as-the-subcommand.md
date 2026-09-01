---
id: '882fea0f3d66d72f'
kind: bug
status: open
title: 'BUG: staging_op reads a detached global-flag value as the subcommand, so `git -C <path> add` loses the stager'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
closed: ''
opened: 2026-09-01
owner: marius
related: []
severity: medium
unverified: 'Impact direction is reasoned, not observed in production: `-` over-refuses (the recoverable direction per a987df96''s own ruling), so this generates FALSE REFUSALS rather than captures. No production false refusal has been observed yet — the mechanism is measured, its live cost is not.'
---

## Summary
`scripts/post-index-change-stage-log.sh`'s `staging_op()` decides whether an index write was
caused by a *staging* command by parsing `/proc/$PPID/cmdline`. It skips flag tokens (`-*`)
but **not the detached values those flags consume**, so the value is read as the subcommand,
falls to `*) return 1`, and the write is recorded as `-` (unattributable) instead of the
staging session.

`git -C <path> add <file>` — the exact form `codescout-companion`'s worktree-ambiguity guard
**mandates** — is therefore never recognised as a staging operation.

## Symptom (Effect)
Same repo, same session id, three invocation forms, one `git add` each:

```
  plain            git add f                                  -> AAAA   correct
  dashC            git -C <path> add f                        -> -      STAGER LOST
  gitdir_equals    git --git-dir=X --work-tree=Y add f         -> AAAA   correct
  gitdir_space     git --git-dir X --work-tree Y add f         -> -      STAGER LOST
  dashc_kv         git -c user.name=x -C <path> add f          -> -      STAGER LOST
```

The equals form survives only by accident: `--git-dir=X` is a single token caught by the
`*=*) continue` arm. Nothing handles a flag whose value is a separate argv token.

## Reproduction
Verified 2026-09-01 in throwaway repos under `$TMPDIR`; the real checkout was never touched.

1. `git init` a temp repo; wire `.git/hooks/post-index-change` to
   `scripts/post-index-change-stage-log.sh`.
2. `CLAUDE_CODE_SESSION_ID=AAAA git -C "$P" add f.txt`
3. `awk -F'\t' '$3=="f.txt"{print $1}' "$P/.git/session-stage-log"` → prints `-`, want `AAAA`.

Step 2 with cwd inside the repo and a bare `git add f.txt` prints `AAAA`, which is the
discriminator: only the invocation form changes.

## Environment
`scripts/post-index-change-stage-log.sh` at `a987df96` (the "stager wins" fix). Shared
checkout `/home/marius/work/claude/codescout`, branch `experiments`, git 2.x, Linux.

## Root cause
`staging_op()` (`scripts/post-index-change-stage-log.sh:99-116`) walks the NUL-separated
argv, skips to `git`, then classifies each following token:

```
case "$_so_tok" in
    -*) continue ;;                                  # skips the FLAG
    *=*) continue ;;                                 # skips --k=v (single token)
    add | rm | mv | restore | apply | update-index | stash) return 0 ;;
    *) return 1 ;;                                   # <- the flag's VALUE lands here
esac
```

`-C` consumes the next token. The loop skips `-C`, then meets `/path`, which matches neither
a flag nor a known subcommand, so it returns 1 on the flag's own argument.

This is `IC-6` (`cluster/addressing-without-an-escape-hatch`) in its **value-read-as-syntax**
form — the "heredoc tell" from CLAUDE.md § *Parsers Over a Namespace*: a token that exists
precisely to be **data** (a path) is classified as **syntax** (a subcommand). The parser owes
a rule for "this flag takes an argument"; it has none, so git's flag grammar is not
representable in it.

Git global flags taking a detached value: `-C <path>`, `-c <name>=<value>`,
`--git-dir <path>`, `--work-tree <path>`, `--namespace <name>`, `--exec-path <path>`.
`-c` survives because its value contains `=`.

## Evidence
The three-form table above, produced in one run against one temp repo. The `plain` and
`gitdir_equals` rows are the controls — they prove the hook and the session id are working,
so the `-` rows isolate the invocation form as the only variable.

## Hypotheses tried
1. **The session id was not exported.** REJECTED — the same env assignment yields `AAAA` for
   the plain form in the same script.
2. **Deletions/`git rm --cached` specific.** REJECTED — reproduced with a plain `add`.

## Impact, and why it is the recoverable direction
`-` reads as foreign to `pre-commit-foreign-index.sh`, so the guard **over-refuses**: the
legitimate stager is told their own staged file belongs to someone else, and the refusal
cannot name an owner because the owner was never recorded. Per `a987df96`'s own ruling
(*"unknown over-refuses recoverably where mine under-refuses silently"*) this is the safe
direction — but it is still wrong, and it is aimed squarely at automated callers, which are
the ones that use `git -C`.

**Two guards in this repo disagree.** `codescout-companion`'s worktree-ambiguity hook
*blocks* a bare `git commit` and instructs `git -C /full/worktree/path commit ...`; this hook
then cannot attribute anything staged that way. Following one guard defeats the other. That
collision is the finding, not the parse bug alone.

## Fix
Not attempted. The shape: teach `staging_op()` which global flags consume a following token
and skip both, e.g. a `-C|-c|--git-dir|--work-tree|--namespace|--exec-path)` arm that sets a
skip-next flag. Keep `*=*` for the joined form.

Whatever lands must be pinned by a case per invocation form — a single `git add` case is
monotone under this defect and passes against the broken parser (see below).

## Tests added
None yet. `tests/hooks-discrimination.sh` § *stager wins* asserts attribution **only** via
plain `git add` / `git rm --cached`, so all 21 cases pass against the defect. This is CLAUDE.md
§ *Testing Discipline*'s "mutate once per guarded SITE, not once per feature": the site here
is the invocation form, and one form was sampled.

## Workarounds
Run staging commands with the cwd inside the repo (`cd "$P" && git add f`), or use the joined
`--git-dir=`/`--work-tree=` form, which parses correctly.

## Resume
Open. Mechanism measured and reproduced; fix and per-form test cases owed.

## References
- `scripts/post-index-change-stage-log.sh:99-116` (`staging_op`)
- `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` — the fix (`a987df96`) this defect survives
- `tests/hooks-discrimination.sh` § *stager wins* — the suite that passes against it
- `docs/issues/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md` — found in the same pass

