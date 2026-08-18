---
status: open
opened: 2026-08-18
closed:
severity: medium
owner: marius
related: []
tags: [grep, cross-repo, false-negative]
kind: bug
---

# BUG: `grep(glob=<absolute path outside the project>)` returns a silent zero, and the warning names the wrong cause

## Summary

Passing an absolute path *outside the active project root* as `grep`'s `glob`
argument returns `0 matches` instead of an error. The accompanying warning
attributes the zero to unsearched hidden directories — a plausible but wrong
cause, with a suggested fix (`include_hidden=true`) that cannot help. The same
file searched via the `path` argument matches correctly, so the tool is
inconsistent between its two targeting parameters.

## Symptom (Effect)

Searching a sibling repo's file for a pattern it demonstrably contains:

```
grep(pattern="^export function|^function |spawnSync|execFileSync",
     glob="/home/marius/work/claude/claude-plugins/codescout-companion/hooks/lib.mjs",
     mode="content")
→ 0 matches

warning: this zero describes what was searched, not the pattern. Hidden paths
were not searched, including .buddy/, .cargo/, .claude/, .fastembed_cache/,
.github/, .superpowers/, .worktrees/, .env and 8 more at the search root.
Pass include_hidden=true to search them — a glob cannot re-admit them, because
overrides are applied inside a walk that has already pruned the parent directory.
```

The file contains 8 `export function` declarations and imports `execFileSync`,
confirmed by shell:

```
$ grep -nE '^export (function|const)|spawnSync|execFileSync' .../hooks/lib.mjs
12:import { execFileSync } from 'node:child_process';
16:export function readInput() {
25:export function emit(obj) {
30:export function denyPreToolUse(reason) {
41:export function contextPreToolUse(context) {
52:export function detectFor(cwd) {
66:export function git(cwd, args) {
80:export function emitSkillHint(cwd, sessionId, topic, hint) {
```

## Reproduction

Commit: `b4398a6a` (branch `experiments`). Active project: codescout.

1. `grep(pattern="export function readInput", glob="<abs path to a file in a sibling repo>", mode="content")`
   → `0 matches` + the hidden-paths warning.
2. `grep(pattern="export function readInput", path="<the same abs path>", mode="content")`
   → matches: `16: export function readInput()`.

Step 2 is the control: it proves the pattern, the file, and cross-repo reads
are all fine, isolating the defect to `glob`.

## Environment

Linux, codescout MCP over stdio, project `/home/marius/work/claude/codescout`,
branch `experiments`. Target file lives in a *different* repo
(`/home/marius/work/claude/claude-plugins/`) that is not an active workspace
member.

## Root cause

`Unknown — under investigation` at the mechanism level; the *boundary* is
measured. `glob` patterns are matched against paths produced by a walk rooted
at the active project, so an absolute path outside that root can never match
any candidate — the walk never yields a path with that prefix. `path` takes a
different route that resolves the target directly and therefore escapes the
root.

measured 2026-08-18: the two-call A/B above (`glob` → 0, `path` → 1 match on
the same file and pattern) — behaviour observed at runtime, not read out of
the source. The specific call sites have not yet been read, so the claim above
is the observed boundary, not a cited mechanism.

The second, independent defect is the **warning text**: it is emitted on any
zero and asserts a cause (hidden-path pruning) that it has not checked applies.
Here it was wrong, and its remedy (`include_hidden=true`) would have produced
another zero. A warning that guesses is worse than no warning, because it
terminates the search for the real cause.

## Evidence

### The A/B pair, same session

Both calls in this session, minutes apart, same pattern family, same file:
`glob=` → `0 matches` + hidden-paths warning; `path=` → `16: export function
readInput()  [readInput]`. Nothing about the file changed between them.

### Why it mattered here

The zero arrived mid-reconnaissance while scouting the companion plugin's
`lib.mjs` ahead of a Phase B implementation plan for
`docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` §6,
whose hook-side work lives in that exact file. Acting on the zero would have
meant planning a rendezvous hook against a helper library believed to export
nothing.

## Hypotheses tried

1. **Hypothesis:** The pattern was wrong (alternation/anchor mismatch).
   **Test:** Re-ran a single simple pattern, `export function readInput`, via `path=`.
   **Verdict:** rejected — matched immediately.
   **Evidence link:** § The A/B pair.
2. **Hypothesis:** Hidden-directory pruning, as the warning claims.
   **Test:** Target is not hidden and contains no hidden path component;
   `path=` reached it without `include_hidden`.
   **Verdict:** rejected — the warning's stated cause does not apply.
   **Evidence link:** § The A/B pair.

## Fix

Not yet implemented. Two candidates, not mutually exclusive:

- Make `glob` reject an absolute path that does not start with the active
  project root, as a `RecoverableError` naming `path=` (and `workspace(action="activate")`
  for genuine cross-repo work) as the remedy. Preferred: it converts a silent
  false negative into a directed error.
- Narrow the hidden-paths warning so it fires only when hidden pruning could
  actually explain the zero — i.e. when a pruned directory is a prefix of the
  requested target.

## Tests added

None yet — bug is open, no fix written.

## Workarounds

Use `path=<absolute path>` for a single file in another repo; it resolves
correctly today. For a multi-file cross-repo sweep, either
`workspace(action="activate")` that repo first, or fall back to
`run_command` with shell `grep`, which is unaffected.

## Resume

Read `grep`'s argument handling to convert the measured boundary above into a
cited mechanism: find where `glob` is compiled and against which candidate
string it is matched (project-relative vs absolute), and where the
hidden-paths warning is emitted on an empty result set. Confirm whether `glob`
is documented as project-relative — if so, the defect is narrowed to the
missing validation plus the misattributing warning, and the fix is the
`RecoverableError` above.

## References

- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` §6 — the scout that surfaced it
- `docs/trackers/bug-fix-session-log.md` F-55 — the session-log entry
- `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/lib.mjs` — the file that "had no exports"
