---
id: cecb14883dc189ea
kind: bug
status: open
title: A git-verb regex swallows longer subcommands sharing its prefix, so read-only plumbing is refused as a mutation
tags:
- cluster/addressing-without-an-escape-hatch
closed: null
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

Not fixed. Two candidate shapes, neither implemented:

1. **Anchor each alternative** — require the subcommand to be followed by whitespace or end of
   segment, e.g. `(?:commit|push|rebase|merge|…)(?=\s|$)`. Cheapest, and directly expresses "whole
   subcommand". Note `reset\s+--hard` and `checkout\s+-b` already carry their own suffix and would
   need the lookahead placed after it.
2. **Enumerate the exclusions** — add a negative alternative for the known read-only siblings.
   Rejected on sight: it is a closed list against an open namespace, and git adds subcommands.

Candidate 1 is preferred. The class's own rule applies — a parser over a namespace owes a
disambiguator, and the disambiguator here is "the verb ends where the subcommand ends".

## Tests added

None — nothing is fixed. A regression test would assert `git merge-base --is-ancestor A B` passes
the guard while `git merge origin/main` is refused, which is the two-sided pair this class needs:
a one-sided "merge-base passes" test is monotone under the guard never firing at all.

## Workarounds

Use the explicit form the refusal already recommends: `git -C /abs/path merge-base …`. It is
exempted at `:67` and is good practice on a multi-worktree checkout regardless.

## Resume

Decide between candidate 1 and 2 above, then patch
`claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs:65` and add the two-sided test.
The hook is in a sibling repo, not this one — changing it needs a commit there.

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
