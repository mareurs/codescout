---
status: open
opened: 2026-09-03
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/unclassified
kind: bug
---

# BUG: the worktree guard's `merge\b` matches `git merge-base`, so six read-only plumbing commands are blocked as destructive mutations

## Summary

`git-worktree-guard.mjs`'s trigger regex ends each verb with `\b`, and a hyphen is a word
boundary. So `merge\b` matches `git merge-base` and `commit\b` matches `git commit-tree` —
six read-only plumbing commands are refused as *"Worktree-ambiguous git mutation"*. The
guard is correct about every command it was written for; the defect is entirely in what else
the boundary admits.

codescout's own IL-3 refusal message names `merge-base` in its list of *"single-line plumbing
(rev-parse, patch-id, merge-base, describe)"* that is **always** safe. Two guards in the same
process therefore classify the same command in opposite directions.

## Symptom (Effect)

Observed earlier in this session from inside a worktree: a `Bash` chain was refused with the
offending segment named as `git merge-base` — a command that writes nothing and prints one
SHA.

```
⛔ Worktree-ambiguous git mutation. BLOCKED.
```

## Reproduction

**Live reproduction obtained 2026-09-03 (later the same day), profile `.claude`, session
`2cb44cd3-8673-4604-a8ac-5adea75ca54b`.** Native `Bash`, main checkout, 3 worktrees present.
All three refused with *"Worktree-ambiguous git mutation. BLOCKED."*, the `Offender:` line
naming the bare command:

```
git merge-base --is-ancestor 3e8193a0 experiments   # BLOCKED
git merge-base 3e8193a0 experiments                 # BLOCKED  (flag is irrelevant)
git commit-graph verify                             # BLOCKED  (second verb family)
git -C /home/marius/work/claude/codescout merge-base 3e8193a0 experiments   # allowed
```

The first was hit ACCIDENTALLY, mid-reconnaissance, on the ancestry check that verifies a
fix is still on the branch after a rebase — the check `CLAUDE.md` § *Git Workflow*
prescribes. The other three were then run deliberately to find the boundary. This closes
Hypothesis 2 below and upgrades the severity evidence from source-only to observed.

**Two dead ends worth recording, both falsified here:** the `--is-ancestor` flag is NOT the
trigger (bare `git merge-base` is refused identically), and the shell segmenter is NOT the
cause (the first refusal arrived inside a `for` loop with a mis-cut `Offender:` fragment,
which looks exactly like a `segments()` defect; the bare one-line form is refused too).
Both will re-suggest themselves to the next reader of that `Offender:` line.

**Why the earlier probes could not reach it, recorded rather than worked
around:** the `pre-tool-guard` shell redirect (*"codescout offers a leaner path for shell
work"*) now intercepts `Bash` before `git-worktree-guard.mjs` is reached, and the worktree the
original refusal fired in has since been removed. Two probes on 2026-09-03 — `git merge-base
HEAD HEAD` from the main checkout, and `git -C <peer worktree> merge-base HEAD HEAD` — both
returned the shell redirect, not the worktree guard.

The mechanism is instead established **at the source**, which is stronger than the symptom:

`claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs:65`

```js
const TRIGGER = /git\s+(commit|push|reset\s+--hard|rebase|merge|checkout\s+-b)\b/;
```

`\b` matches between a word character and a non-word character. `-` is a non-word character,
so `merge\b` matches the `merge` in `merge-base`. Every hyphenated git subcommand whose stem
is a listed verb is therefore a trigger:

| command | matches via | what it actually does |
|---|---|---|
| `git merge-base` | `merge\b` | prints a commit SHA; writes nothing |
| `git merge-tree` | `merge\b` | prints a merge result; writes nothing |
| `git merge-file` | `merge\b` | writes only the file named on the command line |
| `git merge-index` | `merge\b` | runs a merge program per unmerged path |
| `git commit-tree` | `commit\b` | prints a commit object id; moves no ref |
| `git commit-graph` | `commit\b` | writes a local cache file; moves no ref |

`reset\s+--hard`, `push` and `checkout\s+-b` have no hyphenated collisions, so the blast
radius is exactly the `merge` and `commit` rows.

## Environment

Linux. `claude-plugins/codescout-companion`, hook `git-worktree-guard.mjs`. Fires only when
the repo has ≥2 worktrees (single-worktree carve-out at `:93`), which this checkout has had
throughout.

## Root cause

`\b` is the wrong delimiter for a command-name boundary. A git subcommand name ends at
whitespace or end-of-segment, not at any non-word character, and `-` is the one non-word
character that routinely appears *inside* a subcommand name.

This is a real cost rather than a theoretical one, because the guard's failure mode is
asymmetric by design and correctly so: the file's own comment says quote-naive splitting
*"fails toward blocking, never toward allowing"*. That reasoning is right for the splitter,
where the alternative is a silent unguarded `git commit`. It does not extend to the verb
boundary, where the alternative is not a missed mutation but a refused read.

**The guard is unusually well-hardened otherwise**, which is what makes this worth filing
rather than shrugging at: heredoc bodies are stripped (`:20`), detection is per-segment rather
than over a flat blob, and the `cd` exemption is deliberately forward-only. All three came
from a prior bug
(`docs/issues/archive/2026-09-01-worktree-guard-scans-the-whole-command-so-a-heredoc-blocks-and-a-mention-disarms.md`).
Every one of those fixes was about *which text* the verbs are matched against. None was about
what the verb pattern itself matches, so the hardening pass could not have surfaced this.

## Evidence

### Two guards in one process disagree about the same command

codescout's IL-3 refusal message (`src/`, emitted on every blocked pipe) reads:

```
Single-line plumbing (rev-parse, patch-id, merge-base, describe) is always bounded.
```

`merge-base` is named there as the canonical safe read. The worktree guard classifies the same
string as a destructive mutation. Neither is wrong about its own concern — IL-3 is about output
size, the worktree guard about ambiguous writes — but an agent that reads both has been told
`merge-base` is both always-safe and blocked, and nothing reconciles them.

### The workaround is real but wrong-shaped

`EXPLICIT_C` at `:67` exempts `git -C <path> <verb>`, so `git -C /abs/path merge-base …` is
allowed. That is the documented escape and it works. But it asks the caller to *disambiguate a
worktree* for a command that reads a ref and touches no working tree — the exemption's premise
does not apply to the command it is rescuing.

## Hypotheses tried

1. **Hypothesis:** the refusal seen earlier came from a different guard.
   **Test:** grep the hook directory for the refusal string.
   **Verdict:** rejected. `git-worktree-guard.mjs:99` is the only source of *"Worktree-ambiguous
   git mutation. BLOCKED."*, and `:1` describes exactly that intent.

2. **Hypothesis:** the symptom can be re-triggered on demand today.
   **Test:** two `Bash` probes, from the main checkout and against a peer worktree.
   **Verdict:** rejected AT THE TIME, then **confirmed later the same day** from a different
   profile — see § *Reproduction*. The interception was real and the denominator was the right
   call; what it turned out to measure is the PROFILE, not the bug. A `.claude-sdd` session
   reaches the shell redirect first and a `.claude` session reaches the worktree guard, so
   "not reproducible" was true of the observer rather than of the defect. That sharpens the
   warning already written here: a re-checker does not merely see the wrong refusal — they see
   a DIFFERENT one depending on which profile they run, and neither refusal names the profile
   as the variable.

## Fix

Anchor the verb at a real command boundary — whitespace or end-of-segment — rather than `\b`:

```js
const TRIGGER = /git\s+(commit|push|reset\s+--hard|rebase|merge|checkout\s+-b)(\s|$)/;
```

`reset\s+--hard` and `checkout\s+-b` already end in a token that cannot be a stem, so only
`commit`, `push`, `merge` and `rebase` change behaviour, and only for hyphenated successors.

**Verify with a table, not a spot check** — the six rows above must pass and the four bare
verbs must still block. A single `git merge-base` probe would go green under a regex that had
broken `git merge` entirely, which is the mutation that matters here.

SHA: *pending.* patch-id: *pending.*

## Tests added

None yet. The hook has no test harness in this repo; the fix should carry the six-plus-four
table as a fixture wherever the companion plugin's tests live.

## Workarounds

Use the documented escape — `git -C <abs path> merge-base …` — or call the command through
`run_command`, which this guard does not gate (it fires only on `tool_name === 'Bash'`,
`:57`).

## Resume

Apply the one-character-class fix at
`claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs:65` and verify against the
six-row table above plus the four bare verbs. Note the repo boundary: the fix lands in
`claude-plugins`, not codescout, so it needs that repo's own gate and its SHA cited with the
`<repo>:<sha>` prefix.

**Cluster:** filed `cluster/unclassified` deliberately. `IC-14`
(`guard-narrower-than-its-name`) is the nearest class and is its **inverse** — this guard's
coverage is *wider* than its name, refusing work rather than missing it. Forcing it into
`IC-14` would corrupt the count that class's promotion reads, which the ledger explicitly warns
against. If a second over-matching guard turns up, the candidate class is *"a guard's trigger
matches a superset of what its name claims"*, and that is when to declare it — not now, at
n=1.

## References

- `claude-plugins/codescout-companion/hooks/git-worktree-guard.mjs` — `:1` intent, `:65`
  `TRIGGER`, `:67` `EXPLICIT_C`, `:93` single-worktree carve-out, `:99` refusal text.
- `docs/issues/archive/2026-09-01-worktree-guard-scans-the-whole-command-so-a-heredoc-blocks-and-a-mention-disarms.md`
  — the prior hardening pass, which fixed *which text* is matched and not *what the verbs
  match*.
- `docs/architecture/companion-plugin.md` — hook inventory and cross-repo flow.
