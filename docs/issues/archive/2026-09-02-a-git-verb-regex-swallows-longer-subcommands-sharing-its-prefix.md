---
id: 54c25b97fdea21c2
kind: bug
status: fixed
title: A git-verb regex swallows longer subcommands sharing its prefix, so read-only plumbing is refused as a mutation
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
fix_patch_id: 1ec8d70c31e5df900096617793b9b0fa17a0abb5
fix_sha: claude-plugins:7a95dffb6ee7da00d7a79c74919aabc02dcb98b7
fixed: 2026-09-09
opened: 2026-09-02
owner: marius
severity: low
---

## Summary

The worktree-ambiguity guard's trigger regex matches its own verb as a *prefix* of longer git
subcommands, so read-only plumbing is refused under a banner that calls it a mutation.
`git merge-base --is-ancestor A B` — which writes nothing — is blocked as a *"Worktree-ambiguous
git mutation"*. Same mechanism reaches `git commit-graph`, `git merge-file`, `git merge-tree`,
`git merge-index`.

## Symptom (Effect)

```
⛔ Worktree-ambiguous git mutation. BLOCKED.

Command : git merge-base --is-ancestor 1559daa5 a8d2e0d9 && echo "YES" || echo "NO"
Offender: git merge-base --is-ancestor 1559daa5 a8d2e0d9
CC PWD  : /home/marius/work/claude/codescout
Worktrees (4):
...
```

`git merge-base` is a read-only query. It reports whether one commit is an ancestor of another
via its exit status and writes nothing — no ref, no index, no object.

## Reproduction

In any checkout with ≥2 worktrees (`wtCount < 2` exits early, so a single-worktree repo cannot
reproduce), from the Bash tool:

```
git merge-base --is-ancestor <sha-a> <sha-b> && echo yes || echo no
```

Observed at `5eea9301`. `git -C <path> merge-base …` passes, because `EXPLICIT_C` exempts it.

## Environment

Linux; `codescout-companion` plugin hook, all three CC profiles. Fires on the native `Bash`
tool. `run_command` is unaffected — it has its own gates.

## Root cause

**Measured 2026-09-02** by reading the hook, not inferred from behaviour.
`/home/marius/work/claude/claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs:65`:

```js
const TRIGGER = /git\s+(commit|push|reset\s+--hard|rebase|merge|checkout\s+-b)\b/;
```

The `\b` sits *after* the alternation group, so it only asserts a word boundary at the end of
whichever alternative matched. For `git merge-base`, the `merge` alternative matches and the
following character is `-` — a non-word character — so `\b` **succeeds**. The regex has no way to
say *"`merge` as a whole subcommand, not `merge` as a prefix"*, because the token that would
terminate it (`-`) is exactly the token git uses to build longer subcommand names.

The same holds for every alternative whose verb is a prefix of a real subcommand:

| written | also matches | read-only? |
|---|---|---|
| `merge` | `merge-base`, `merge-file`, `merge-tree`, `merge-index` | `merge-base` yes; `merge-tree` yes (since git 2.38 it can write, but not by default) |
| `commit` | `commit-tree`, `commit-graph` | `commit-graph verify` yes |

`push`, `rebase`, `reset --hard` and `checkout -b` have no such collision today.

## Evidence

Word-boundary semantics, first principles: in `merge-base`, the boundary between `e` (word) and
`-` (non-word) is a `\b` position. Any anchor-after-alternation regex over a namespace whose
members share prefixes has this property; it is not specific to this pattern's authorship.

The escape route works and is documented in the refusal text itself — `git -C <path> merge-base …`
passed immediately, exempted by `EXPLICIT_C` at `:67`.

## Hypotheses tried

1. **Hypothesis** — the guard deliberately includes read-only commands for safety.
   **Test** — read the hook's own comments. **Verdict** rejected. `:64` reads *"Destructive git
   verbs (bare `git checkout <ref>` is read-mostly, skipped)"*, and the refusal text at `:108`
   enumerates *"commit/push/reset/rebase/merge/checkout -b"* as commands that "land on whatever
   branch CC's PWD points at". `merge-base` lands nothing. The inclusion is accidental.

## Fix

Fixed by **candidate 1** (anchor each alternative) in `claude-plugins:7a95dffb6ee7da00d7a79c74919aabc02dcb98b7`,
patch-id `1ec8d70c31e5df900096617793b9b0fa17a0abb5`. The fix lives in a **sibling repo**, which is why
no codescout gate ever reddened while this file sat `open`.

`hooks/git-worktree-guard.mjs` now reads:

```js
const TRIGGER = /git\s+(commit|push|reset\s+--hard|rebase|merge|checkout\s+-b)(\s|$)/;
```

The boundary moved from `\b` to `(\s|$)`. A hyphen is a non-word character, so `\b` *succeeded*
between `merge` and `-base`; whitespace-or-end does not, which is the disambiguator the class asks
for — "the verb ends where the subcommand ends". Candidate 2 (enumerate exclusions) was rejected as
filed: a closed list against an open namespace.
## Tests added

`claude-plugins:tests/test-git-worktree-guard.sh` — **two-sided**, as this file's plan required:

- **Allow side** (`:104-123`): six hyphenated read-only plumbing commands — `merge-base`,
  `merge-file`, `merge-tree`, `merge-index`, `commit-tree`, `commit-graph`.
- **Deny side** (`:29`): the pre-existing bare-mutation block, which is what keeps the allow side
  from being monotone under "the guard never fires at all".

The fixture carries its load-bearing detail on the block itself — *"The anchor is `(\s|$)`; if it
regresses to `\b`, every case here flips"* — so a tidy-up that removes the annotation cannot leave
a passing-but-no-longer-discriminating test unremarked.

Suite verified 2026-09-09: **38 passed, 0 failed**.
## Workarounds

Use the explicit form the refusal already recommends: `git -C /abs/path merge-base …`. It is
exempted at `:67` and is good practice on a multi-worktree checkout regardless.

## Resume

Nothing outstanding. Fix and two-sided regression test are both live in `claude-plugins`; verified
2026-09-09 by running the suite (38/38) and by evaluating the shipped `TRIGGER` regex against all
11 named cases (6 allow, 5 deny) — all correct.
## References

- Hook: `claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs:61-113`
- Class: `docs/trackers/issue-clusters.md` — `IC-6`, `cluster/addressing-without-an-escape-hatch`,
  whose disambiguator half this instantiates. A recent sibling member,
  `docs/issues/archive/2026-09-01-staging-op-reads-a-detached-flag-value-as-the-subcommand.md`,
  is the same defect one layer over: a git-subcommand parser mis-reading its own namespace.
- Noticed while deriving SHA/patch-id pairs during the entry-id cross-host collision plan; recorded
  by a peer in `docs/issues/archive/2026-09-02-worktree-guard-refuses-writes-and-lets-unpinned-reads-through.md`
  § References as adjacent and separately filable, deliberately not folded in — a different guard
  and a different defect.
