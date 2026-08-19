---
kind: bug
status: fixed
tags:
- grep
- cross-repo
- false-negative
closed: 2026-08-19
opened: 2026-08-18
owner: marius
related: []
severity: medium
unverified: 'the second defect named in the title is NOT fixed in general: the hidden-paths completeness warning still asserts its remedy without checking that hidden pruning could explain the zero. The reported misattribution can no longer occur, because the glob case now errors before any walk runs, but the narrowing candidate in Fix remains unimplemented.'
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

**Now cited, not inferred.** The Resume asked for the measured boundary to be converted
into a mechanism read out of the source; this is that.

`src/tools/grep.rs`: the walk is built as `ignore::WalkBuilder::new(&search_path)`, and
when globs are present they are compiled into
`ignore::overrides::OverrideBuilder::new(&search_path)`. Overrides filter the candidates
**the walk yields**, and every one of those carries `search_path` as a prefix. An absolute
pattern outside that root is therefore unsatisfiable by construction — not "unlikely to
match", but incapable of matching any string the walk can produce. The walk then completes
normally, so the result is a well-formed `0 matches` rather than an error.

`search_path` is `validate_read_path(raw_path, …)`, i.e. the resolved `path` argument, or
the project root when `path` is absent. So the boundary is the **search root**, not the
project root — which is why `path=<abs path>` works: it makes the target the root instead
of filtering a walk over a different one. That is exactly what the A/B pair measured, and
the mechanism explains why the two parameters disagree.

The second, independent defect stands as filed: the **warning text** is emitted on any
zero where hidden entries exist at the root, and asserts a remedy
(`include_hidden=true`) it has not checked applies. Here it could not have helped. A
warning that guesses is worse than no warning, because it terminates the search for the
real cause.
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

**The first candidate, implemented.** `unsatisfiable_absolute_glob(globs, search_path)` in
`src/tools/grep.rs` returns the first glob that is an absolute path outside the search
root; `Grep::call` refuses with a `RecoverableError` naming both remedies — `path=<abs
path>` for a one-off, `workspace(action="activate", …)` for sustained cross-repo work.
The check runs immediately after `parse_globs`, before the regex is built or any walk
starts, so nothing is searched on the way to the error.

Three conditions, each load-bearing and each mutation-checked:

- **`is_absolute()`** — without it every relative glob (`*.rs`) is "outside the root" and
  every ordinary grep breaks. This is the mutation with the widest blast radius.
- **`!starts_with(search_path)`** — without it an absolute glob *inside* the root is
  rejected, though it shares the prefix every candidate carries and matches fine.
- **the call-site guard** — without it the predicate is computed and discarded, which is
  this subsystem's recorded failure mode rather than a hypothetical one.

A negation (`!…`) is not an absolute path, so gitignore-style negations are untouched.

**The second candidate is deliberately not implemented** — see `unverified:`. Narrowing
the hidden-paths warning to fire only when hidden pruning could explain the zero is a
separate change; what this fix does is remove the case where the warning was measurably
wrong, by erroring before the warning path is reached at all.
## Tests added

Two. `unsatisfiable_absolute_glob_flags_only_absolute_paths_outside_the_root` pins the
predicate across five cases — absolute-outside, absolute-inside, relative, negation, and
an offender that is not first in the list. `grep_rejects_an_absolute_glob_outside_the_search_root`
is end-to-end and **carries the bug's own Reproduction step 2 as its control**: the same
file and the same pattern via `path=` must still match. Without that control the test
would pass just as well if the fix had broken cross-repo reads altogether.

Mutations applied and the **observed** result:

| # | Mutation | Observed |
|---|---|---|
| M1 | remove the call-site guard | end-to-end test FAILS |
| M2 | drop `is_absolute()` | relative-glob case FAILS |
| M3 | drop `!starts_with(root)` | absolute-inside case FAILS |

Zero survivors, one fixture each. **M1's failure output is the bug itself** —
`{"file_groups": [], "total": 0, "files": 0}`, the confident zero about a file that was
never opened.

Gate: fmt, clippy `--all-targets -D warnings`, `cargo test` 4264 passed / 45 ignored
(+2 from 4262).
## Workarounds

Use `path=<absolute path>` for a single file in another repo; it resolves
correctly today. For a multi-file cross-repo sweep, either
`workspace(action="activate")` that repo first, or fall back to
`run_command` with shell `grep`, which is unaffected.

## Resume

The mechanism question this section asked is answered — see § *Root cause*, which now
cites the two constructor calls rather than describing an observed boundary.

One thing remains, and it is the caveat in `unverified:`: **narrow the hidden-paths
completeness warning** so it fires only when hidden pruning could actually explain the
zero. `WalkAudit::completeness_warning` in `src/tools/grep.rs` currently keys on "the
result was empty and the root has hidden entries", which is a fact about the tree rather
than evidence about this query.

Worth measuring before changing, in the same spirit that the backtick gate's blocker was
measured: how often does a zero-match `grep` co-occur with hidden entries at the root that
could not have held the pattern? `.codescout/usage.db` holds the calls. A warning narrowed
on a guess would be the same defect in the other direction.
## References

- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` §6 — the scout that surfaced it
- `docs/trackers/bug-fix-session-log.md` F-55 — the session-log entry
- `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/lib.mjs` — the file that "had no exports"

## Fix provenance

- **SHA:** `c38bfd91` (`experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `913ba9c70e0b4fc97834d91159e56d0579ffd071` — content hash of the diff; survives rebase and cherry-pick.

Covers the first of the two defects in the title. The second is recorded in `unverified:`
and in § *Resume*; there is no second commit owed for it, because it is a change nobody
has decided to make.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 913ba9c70e0b /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several branches
(cherry-pick) and any of them is the fix.
