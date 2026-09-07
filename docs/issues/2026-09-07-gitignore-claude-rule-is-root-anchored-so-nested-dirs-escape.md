---
id: '252fe84782103842'
kind: bug
status: open
title: 'BUG: `.gitignore`''s `/.claude/*` is root-anchored, so hook-created nested `.claude/` dirs are never ignored'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
topic: gitignore pattern anchoring
closed: ''
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

Add an unanchored rule alongside the anchored one, keeping the existing negation working:

```
/.claude/*
!/.claude/skills/
**/.claude/          # hook-created, any depth — see goal-stop-hook.mjs:16
```

The trailing slash restricts it to directories, and it does not re-shadow the root entry
because the root `.claude/` is already matched more specifically. Verify with
`git check-ignore -v` on all three paths in § *Symptom* — the first must still report line 42.

- **SHA (experiments):** pending
- **patch-id:** pending

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

