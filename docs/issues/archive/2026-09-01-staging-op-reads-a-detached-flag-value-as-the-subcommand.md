---
id: e06f747f579eca84
kind: bug
status: fixed
title: 'BUG: staging_op reads a detached global-flag value as the subcommand, so `git -C <path> add` loses the stager'
owners:
- marius
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
unverified: 'Fix and mechanism are measured and pinned (7278508e, 5 regression cases, 3 confirmed RED first). What was never counted is the IMPACT: no production false refusal was ever observed or tallied, so the cost claim stays inferential. Peer codescout-cc confirmed continuous live use of the `git -C <abs>` form during the defect''s lifetime, which establishes exposure, not incidents.'
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

**~~Two guards in this repo disagree.~~ RETRACTED 2026-09-01, same day, by probe.** This
section originally argued that `codescout-companion`'s worktree-ambiguity hook *mandates*
`git -C /full/path`, which this hook then cannot attribute — "following one guard defeats the
other" — and called that collision the real finding, larger than the parse bug.

It is **wrong**. Raised by peer `codescout-68`, who hit both guards in sequence and noticed the
ordering, then confirmed with one command:

```
git add --dry-run docs/RELEASE.md      -> exit 0, NOT blocked
```

The worktree guard refuses the **commit-family** verbs (`commit/push/reset/rebase/merge/
checkout -b`) and never refuses `git add`. Both of the commands it blocked merely *contained*
`git commit`. Since attribution is recorded at **staging** time, the compliant path leaves
staging bare and attributable, and the mandated `-C` lands on the commit — which claims
nothing. No tension exists on the path where attribution happens.

**The population was smaller than the framing implied, too**, and was only measured after the
challenge: `-C` accounts for **32 of 1586** real `git add` invocations (**2.0%**), with **0**
using the separated `--git-dir <path>` form. Not empty — those 32 were genuinely mis-attributed
and `-` is the recoverable direction — so the fix stands on its own merits. What does not stand
is the claim that it resolved a guard collision.

Recorded as `F-90` in `docs/trackers/bug-fix-session-log.md`: being blocked by a guard twice
felt like having tested it, and the claim reached five surfaces — three commit messages, this
file, and a source comment — before anyone probed it. A refusal establishes what a guard
*refuses*, never what it *permits*.

## Fix

**Fixed at `7278508e` on `experiments`** (patch-id `b631c794728005941272b2191a709329644ecccd`).

`staging_op()` now carries a `_so_skip_value` flag and an arm listing the global flags that
consume the **next** argv token — `-C`, `-c`, `--git-dir`, `--work-tree`, `--namespace`,
`--exec-path`, `--super-prefix` — placed **before** the generic `-*)` arm so the joined form
(`--git-dir=X`, one token) still falls through and consumes nothing.

**The list is a hand-maintained subset and says so at the site.** Raised by peer `codescout-cc`
during the fix as this bug's own `IC-6` shape: a closed enumeration over a namespace, which a
seventh value-taking flag silently rejoins. Git exposes no way to ask which of its global flags
take a value, so the enumeration cannot be derived — but it is **safe incomplete**, and that is
the load-bearing property. An unlisted value-taking flag leaves its value to be classified
below, where it hits `*) return 1` and the pair is recorded `-`. That is the **over-refusing**
direction `a987df96` already chose: `-` is loud and recoverable, a wrong claim is silent. A
future git flag can degrade attribution; it cannot fake it.

Still out of scope, unchanged, and still stated in the script header: `git add -A` under a
single session id, where every path reads as yours by construction.
## Tests added

**`tests/hooks-discrimination.sh`** § *stager wins*, five cases at `7278508e` — 21 → 26, all
passing.

Four assert the stager is recorded regardless of invocation form; the fifth is the
discrimination in the other direction. **Three were confirmed RED against the parent commit
before the fix**, failing with `want 'aaaa…' got '-'` — the feature missing, not a typo:

| case | pre-fix |
|---|---|
| `git add f` | passed — the control |
| `git -C <path> add f` | **RED** |
| `git --git-dir=X --work-tree=Y add f` | passed (one token, caught by `*=*`) |
| `git --git-dir X --work-tree Y add f` | **RED** |
| `git -c k=v -C <path> add f` | **RED** |
| `git -C <path> status` must NOT claim | passed — must hold in both worlds |

The last one is what makes the set mutation-resistant rather than merely green: a fix that
swallows one token after **every** flag, or that returns 0 whenever it cannot classify a token,
passes all four positive cases and fails that one.

Mutation discipline per CLAUDE.md § *Testing Discipline* — the guarded **site** here is the
invocation form, and the pre-existing suite sampled exactly one of them, which is why 21 cases
passed against this defect for the whole of its life.
## Workarounds
Run staging commands with the cwd inside the repo (`cd "$P" && git add f`), or use the joined
`--git-dir=`/`--work-tree=` form, which parses correctly.

## Resume

N/A — root cause measured, fixed at `7278508e` (patch-id
`b631c794728005941272b2191a709329644ecccd`), pinned by five regression cases, and archived here
the same day.

Archived via `artifact(action="move")`, which re-keyed the artifact: `882fea0f3d66d72f` →
**`e06f747f579eca84`**, 1 event grafted. Cite the new id; the old one no longer resolves.

The one live citation of the old path — `docs/issues/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md`
§ *References* — was re-pointed in the same commit as the move. The citation population was
established with an **unfiltered** sweep rather than the `--include='*.md' --include='*.rs'`
form the guide flags as silently lossy: measured on this repo, live citers of `docs/issues/`
span `.md`, `.rs`, `.yaml`, `.sh`, `.json`, `.toml`, `.py` and `.yml`, so the narrow filter
would have returned a clean zero for any of the latter six. It happened not to matter here —
but that is a fact about this bug, not about the sweep.

The three commit messages naming the old path (`7c44a605`, `7278508e`, `fc48f829`) are
deliberately **not** repaired: history is not rewritten, and a commit message is a record of
what was true when written.
## References
- `scripts/post-index-change-stage-log.sh:99-116` (`staging_op`)
- `docs/issues/archive/2026-09-01-foreign-index-guard-passed-a-peers-staged-deletion.md` — the fix (`a987df96`) this defect survives
- `tests/hooks-discrimination.sh` § *stager wins* — the suite that passes against it
- `docs/issues/2026-09-01-an-absent-stage-log-makes-the-foreign-index-guard-pass.md` — found in the same pass
