---
id: 2882150c03ce050b
kind: bug
status: fixed
title: 'BUG: `.gitignore`''s `/.claude/*` is root-anchored, so hook-created nested `.claude/` dirs are never ignored'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
topic: gitignore pattern anchoring
closed: 2026-09-07
opened: 2026-09-07
owner: marius
related: []
severity: low
---

# BUG: `.gitignore`'s `/.claude/*` is root-anchored, so hook-created `.claude/` dirs in subdirectories are never ignored

## Summary

`codescout-companion`'s `goal-stop-hook.mjs` creates `.claude/` at **whatever cwd the session
has**, but `.gitignore` only ignores the repo-root one. Nested `.claude/` directories therefore
show up as untracked noise inside tracked documentation directories, indefinitely.

## Symptom (Effect)

```
$ git status --porcelain
?? docs/superpowers/plans/.claude/
?? docs/trackers/issue-clusters/.claude/

$ git check-ignore -v .claude/settings.json
.gitignore:42:/.claude/*	.claude/settings.json

$ git check-ignore -v docs/trackers/issue-clusters/.claude/codescout-companion.log
(no output, exit 1 — NOT IGNORED)
```

Both nested directories contain only `codescout-companion.log`.

## Reproduction

```
git rev-parse HEAD          # 4b30601c at filing
```

1. Run any Claude Code session whose cwd is a subdirectory of the repo (e.g. while working on
   `docs/trackers/issue-clusters/`).
2. Let the Stop hook fire once.
3. `git status --porcelain` → `?? <that subdir>/.claude/`.

Observed 2026-09-07 with two such directories already present.

## Environment

Linux (`ripper`), `experiments` @ `4b30601c`, codescout-companion plugin active.

## Root cause

Two independent facts compose:

- `codescout-companion/hooks/goal-stop-hook.mjs:16-19` — `const dir = join(cwd, '.claude');
  mkdirSync(dir, { recursive: true }); appendFileSync(join(dir, 'codescout-companion.log'), …)`,
  where `cwd = input.cwd || '.'`. `recursive: true` means it **creates** the directory rather
  than only writing into an existing one.
- `.gitignore:42` — `/.claude/*`. The leading `/` anchors the pattern to the `.gitignore`'s own
  directory, so it matches `.claude/…` at the repo root and **nothing** deeper.

The four `.worktrees/*/.claude` instances are covered, but by an unrelated rule
(`.gitignore:133` — `.worktrees/`), which is why the gap presents as exactly two directories
rather than six and reads as an isolated accident.

measured 2026-09-07: the three `git check-ignore -v` invocations quoted above.

## Evidence

### The anchoring is deliberate for the root entry and accidental for the rest

`.gitignore:42-43`:

```
/.claude/*
!/.claude/skills/
```

The negation on the next line is why the root entry is anchored — an unanchored `.claude/*`
would make `!/.claude/skills/` ambiguous about which `.claude` it re-admits. So the root
anchoring solves a real problem and the subdirectory case was simply never in view.

## Hypotheses tried

1. **Hypothesis:** the nested dirs are worktree artifacts already covered by `.worktrees/`.
   **Test:** `find . -type d -name .claude -not -path './.git/*'` plus `git check-ignore -v` on
   each. **Verdict:** rejected — 4 of 6 are under `.worktrees/` and covered; the 2 under `docs/`
   are not.

## Fix

**The obvious form is wrong, and this section recommended it.** It read: add `**/.claude/`
alongside the anchored rule, on the reasoning that a trailing slash restricts it to directories
and the root entry is "already matched more specifically". Both halves are false. `**/.claude/`
matches at every depth **including the root**, and git will not descend into an excluded
directory to reconsider a negation — so it silently kills `!/.claude/skills/` on the line above.

Falsified 2026-09-07 in a scratch repo, four arms, before writing anything to the real one:

| arm | `.claude/skills/…/SKILL.md` | `docs/**/.claude/*.log` |
|---|---|---|
| current rules | shown ✓ | **shown — the bug** |
| `+ **/.claude/` (the old recommendation) | **IGNORED ✗** | ignored |
| `**/.claude/` placed first | **IGNORED ✗** | ignored |
| `+ docs/**/.claude/` | shown ✓ | ignored ✓ |

`git check-ignore -v` on arm 2 blames `.gitignore:3:**/.claude/` directly. **Reordering does not
help** — arm 3 was the first guess and fails for the same descent reason.

**The cost of the wrong form is deferred and silent, which is why it survived review.** It does
not untrack anything: the two tracked files under `.claude/skills/` stay tracked, `git status`
stays clean, and the gate stays green. What breaks is the *next* skill added under
`.claude/skills/` — it would never appear in `git status`, and whoever adds it gets no error.
That is this file's own class pointed the other way: a rule broader than its name, failing by
omission.

**Applied form** (`.gitignore:42-58`), scoped rather than global:

```
/.claude/*
!/.claude/skills/
# ... comment recording the falsification ...
docs/**/.claude/
```

Verified against the real repo after applying: the skill file reports `NOT ignored`,
`.claude/settings.json` still reports `.gitignore:42`, both plugin logs report
`.gitignore:58:docs/**/.claude/`, and `git ls-files -- '.claude/skills/*'` still returns 2.

If plugin logs ever appear outside `docs/`, add a sibling scoped line. Do not widen to `**/`.

**Credit:** the falsification is `ad379a7c-a0cf-4c61-bcdb-f0696fea8c30`'s — they tested the three
arms and offered the scoped alternative before this file's author wrote the broken form to disk.
Independently reproduced by `89d91024-cd66-4361-9300-c55b87b179ea` rather than taken on trust,
since it contradicted an already-approved plan.

- **SHA (experiments):** `3182c61c`
- **patch-id:** `370da6629053d1a65af9b2a419409be2a6159733`

Regression test: `tests/hook_config.rs` — `the_claude_skills_negation_survives_the_nested_log_rule`,
`hook_created_claude_dirs_under_docs_are_ignored`, `the_ignore_verdict_helper_discriminates`.
Mutation matrix run against the **production path** (`.gitignore` itself), not the test's inputs:
widening to `**/.claude/` reds the negation test; deleting the scoped rule reds the log test;
deleting `!/.claude/skills/` reds the non-vacuity guard. Each mutation reds exactly one, and the
first two are monotone in opposite directions — either alone would miss the other's regression.

**The first version of that test was vacuous, and the mutation run is what caught it.** It
asserted over `git ls-files -- .claude/skills/`, but `git check-ignore` consults the index, so a
**tracked** path answers "not ignored" whatever the patterns say — measured with the broken rule
in place: tracked file default exit 1, same file `--no-index` exit 0 blamed on `**/.claude/`. The
population was wrong as well as the flag: the harm is the *next* skill added, which is untracked.
It now probes a path nobody has created. Recorded here because a test that passes its own
mutation is the failure this repo keeps paying for.
## Tests added

None. A `git check-ignore` assertion is plausible in `tests/hook_config.rs`, but the failure is
cosmetic and self-announcing in `git status`; noting it here rather than adding a gate whose
cost exceeds the defect.

## Workarounds

Delete the directories (`rm -rf docs/**/.claude`); they contain only a best-effort log and are
recreated harmlessly. Or add the paths to `.git/info/exclude` locally.

## Resume

Edit `.gitignore` near line 42 to add the `**/.claude/` rule shown in § *Fix*, then run the
three `git check-ignore -v` commands from § *Symptom* and confirm the root path still resolves
to line 42 while the two nested paths now resolve to the new rule.

## References

- `.gitignore:42-43` (root entry + negation), `.gitignore:133` (`.worktrees/`)
- `codescout-companion/hooks/goal-stop-hook.mjs:16-19` — the writer
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md`
