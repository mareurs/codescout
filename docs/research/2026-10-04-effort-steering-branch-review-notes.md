# Effort steering: branch review notes

**Date:** 2026-10-04
**Branch:** `feat/effort-steering-core` in the `claude-plugins` repo (worktree `claude-plugins-effort-steering`). Merged locally into `main` on 2026-10-05 as a fast-forward (`main` is now `7475f6d`, with the full suite green on the merged result: 65 suites, 0 failures). Not pushed: local `main` is 19 commits ahead of `origin/main`, two of them unpushed commits from another session. The branch and its worktree still exist, because the worktree is not under `.worktrees/` and holds the scratch evidence.
**Plan:** `docs/superpowers/plans/2026-10-04-effort-steering-core-and-spikes.md`
**Spec:** `docs/superpowers/specs/2026-10-04-effort-steering-design.md` (see its section "Constraints carried into the second plan")
**Findings:** `docs/research/2026-10-04-effort-steering-spike-findings.md`

**Status 2026-10-07:** this document records the first phase (2026-10-04 and 05). The live work list is the tracker `effort-steering-second-phase` in the `claude-plugins` repo (`docs/trackers/effort-steering-second-phase.md`, id `af4ccc8b94a95c91`), with the passover `ec8f1f91d436b653`. The first phase and the hook adapter are on `origin/main` of `claude-plugins`; the rulings below are unchanged.

This document keeps what a gitignored scratch ledger held during the build: the commit map, the review outcomes, the triage of every deferred finding and every decision the controller took on the operator's behalf.

## What was built

Tasks 1 to 10 of the plan are done, each with a task review and a fix loop where the review found problems. Task 11 (the first rules) is held by the operator's decision at the Task 5 gate. `core/rules.json` ships empty.

| Task | What | Commits (`claude-plugins`) |
|---|---|---|
| 1 | Prompt features, test discovery in `tests/run-all.sh` | `2e7757d`, `c0e3cd6` |
| 2 | Spike S2, transcript usage | `13273ea` |
| 3 | Spike S3, offline replay | `1b32e2e`, `e0a355c`, `e578c9f` |
| 4 | Spike S1, hook input dump (live run by the controller, no commit) | `c243591` |
| 5 | Findings and gate (codescout repo) | codescout `853b0108` |
| 6 | Rules engine | `d55c444`, `a2541ed` |
| 7 | Steering text | `539473a` |
| 8 | Deterministic holdout arm | `0e1a9b2`, `715ca1a` |
| 9 | Per-session decision log | `b0b5eb4` |
| 10 | `effort-policy` CLI | `36b6a23`, `80b69aa` |
| Final | Whole-branch review fix wave | `acca20a`, `7475f6d` |

Review outcomes worth keeping:
- Opus reviews found real gaps that Sonnet-tier checks would have missed. In Task 3 they found that about 25% of attributed thinking tokens followed records no human prompt caused. In Task 6 they found the fail-open result and the "ultrathink always wins" rule unpinned for long prompts. In Task 8 they found a surviving divisor mutant. In Task 10 they found that a wrong-shape rules file was invisible in the log.
- The final whole-branch review (Opus) ran the full suite (65 suites, all passed, including all 9 effort-steering suites) and rated the branch "With fixes". It found two must-fix defects: `buildRecord` copied `features` unfiltered (a string-valued feature would reach disk), and the `--flag=value` form was silently ignored. Both are fixed in `acca20a`.

## Deferred findings and their triage

The final reviewer triaged every deferred Minor finding. Items marked FIXED were fixed in the final wave. Items marked LEAVE stay open on purpose.

| Finding | Verdict |
|---|---|
| T1: `explicitDeep` leading word boundary untested | FIXED earlier (ruling R11) |
| T1: README cites the spec by commit SHA | FIXED (`acca20a`) |
| T1: `isQuestion` and `words` whitespace cases; `startsWithSlash` class mutations; timing-based linear test; `isQuestion` comment | LEAVE: unspecified corners or a large margin (about 90 times) |
| T2: unused export, duplicated grouping, `parseFailures` name, `SKIP_SUBTREES` in one function, mutation corners | LEAVE: throwaway spike, results recorded |
| T3: synthetic replies count as zero-thinking turns; untested prompt-text exclusions; `byEffort` unknown; empty bucket prints 0; hard-coded "26 of 59"; cross-file duplicates; R9 counts deduped messages; comment drift | LEAVE: spike. The findings document already caveats the empty-bucket and synthetic items |
| T4: S1 test gaps | LEAVE: the spike did its one job |
| T6: non-boolean `prevTurnFailed` passes the shallow guard | FIXED (ruling R20) |
| T6: `decide` trusts uncompiled rules | FIXED |
| T6: catastrophic regex passes the compile probe | LEAVE in code; recorded as a constraint in the spec |
| T6: probe parameters, `ruleLabel`, file-level skip shape, `isFinite` guards, a pointless wrapper, a compiled `reason` never surfaced | LEAVE: low value |
| T7: `Array.isArray(reasons)` guard unpinned | FIXED |
| T7: inherited-property check unpinned | LEAVE: a table parsed from JSON cannot reach it |
| T8: share-test bands about ±0.05 wide | LEAVE: the strict boundary test and the literal vector pin the recipe |
| T8: wrong comment about a unit value of 0 | FIXED |
| T8: unspecified corners; duplicate test | LEAVE |
| T9: `features` copied unfiltered | FIXED (`acca20a`) |
| T9: lower bound of the id length; umask-dependent mode test; UTF-8 hash vector; Windows reserved names; a pre-planted symlink | LEAVE: Linux, single-user state directory, session ids are lowercase UUIDs |
| T9: adapter must not derive ids from prompt text | Recorded as a constraint in the spec |
| T10: `--flag=value` ignored | FIXED (rulings R19, R22) |
| T10: FIFO as `--rules`; no wall-clock bound | LEAVE in code; recorded in the spec (adapter timeout) |
| T10: five unpinned corners | LEAVE: `internal_error` needs a fault-injection seam |
| Final: reason vocabulary drift (`error` against `internal_error`) | FIXED (ruling R21) |
| Final: exit code 2 for a usage error | FIXED (ruling R18) |
| Final: run-all glob one level deep; slow-regex nondeterminism; spikes on a mergeable branch | Recorded as constraints in the spec |

## Final re-review of the fix wave

The scoped Opus re-review of `80b69aa..7475f6d` found all nine findings addressed and no new Critical or Important breakage. It ran the privacy probe (a string feature plus the slash-prefixed secret prompt), a table of 15 flag forms, the guard with nine `prevTurnFailed` values and the key mutations. The scope check showed exactly the nine named files changed, `rules.json` still empty and the full suite at 218 `ok` and 0 `not ok`.

Findings parked, not fixed (the process allows no second fix wave):
- `bin/effort-policy.test.mjs:549-550`: the test for "a name that is only a prototype key" uses `--constructor` and `--__proto__=x`. Those never reach `Object.prototype`, so the `Object.hasOwn` guard is untested (replacing it with `in` leaves the suite green). Use bare `constructor`, `__proto__=x` and `toString`.
- `bin/effort-policy.test.mjs:418-420`: a stale R19 comment says a trailing `--holdout-rate` "is still just ignored". R22 replaced that, and the correct comment sits right below it.
- `bin/effort-policy.test.mjs:437` and `:569`: two tests check the same input and expectation.
- `core/log.mjs` `safeFeatures` filters by value, not by key. A feature key built from prompt text would pass. `extractFeatures` has fixed keys today. Revisit if a feature is ever keyed dynamically.
- Wording nit: the reason-list header calls `internal_error` "alone in the line", although flags may have been parsed before the failure.

Process slip, recorded for honesty: the re-reviewer ran an old mutation script from the shared scratchpad by mistake. The script mutated and restored `effort-steering/spikes/s3-replay.mjs` in the checkout. The review was meant to be read-only. The controller verified afterwards that the worktree is clean at `7475f6d`, with no diff against HEAD and no change to the protected paths.

## Rulings the controller made on the operator's behalf

Each entry gives the ruling, the reason and what it costs if wrong. The operator may overrule any of them.

| ID | Ruling | Cost if wrong |
|---|---|---|
| R1 | Task 4 was split: an implementer did Steps 1 to 4 and committed. The controller ran the live check (Steps 5 to 7). | A small extra commit |
| R2 | The controller ran Task 5 itself, in the codescout repo. | A little controller context |
| R3 | An invalid `--holdout-rate` is treated as 0 (every turn steered), with the reason `bad_holdout_rate`. The final reviewer preferred rate 1 (no steering on an error). **Still open for the operator.** | Both choices lose those rows. One constant to change |
| R4 | The worktree is a sibling directory, not `.worktrees/` inside the repo. | Move the worktree |
| R5 | The worktree was created without a separate consent question. | `git worktree remove`; `main` is unaffected |
| R6 | Task 3 gained an `effort` field per turn and a per-effort report. | One field and one table to remove |
| R7 | The Task 3 review ran on Opus, not Sonnet. | Extra review cost |
| R8 | The S3 default input is all three profiles' codescout transcripts, with one shared `message.id` set. | Narrow the default directories |
| R9 | A command echo that arrives before the first reply stays in the typed prompt's turn. | One condition to revert |
| R10 | Task 3's two fix rounds got one re-review over the whole range. | A defect could hide between the rounds; the reviewer saw both diffs together |
| R11 | Task 6 also added `explicitDeep` negatives to the Task 1 test file. | Two test lines |
| R12 | A guard-demoted or tied result keeps confidence `low`. | Two confidence values and their tests |
| R13 | `assignArm` throws `TypeError` for non-string ids; the rate is validated first. | One check and one test |
| R14 | A relative `XDG_STATE_HOME` is ignored (the XDG spec says so). | One condition and one test |
| R15 | CLI: default rules and table paths resolve relative to the script, not the current directory; an unusable file gives a reason and version 0. (Its exit-code part is replaced by R18.) | Small edits in one file |
| R16 | The Task 10 review ran on Opus. | Extra review cost |
| R17 | A rules or table file that cannot be used (read failure, parse failure, wrong shape) gives `*_unreadable` and version 0. No new reason name. | Rename a reason string |
| R18 | An unknown or missing subcommand exits 64, not 2. Exit 2 from a `UserPromptSubmit` hook blocks the user's prompt in Claude Code. | One constant |
| R19 | The CLI accepts `--rules=V`, `--table=V` and `--holdout-rate=V`. Any unconsumed token adds `unknown_arg` once. | One parser branch |
| R20 | `prevTurnFailed` counts as a failure unless it is `undefined` or `false`. | One expression |
| R21 | One reason name, `internal_error`, in `decide` and in the CLI. | Rename a string and its tests |
| R22 | A value flag without a value is treated like the empty `=` form (`--holdout-rate` gives `bad_holdout_rate`). | One parser branch |

## Decisions the operator took

- Approved the spec and the plan, and chose subagent-driven execution.
- Approved one scratch `claude -p` call for spike S1.
- At the Task 5 gate: build Tasks 6 to 10 and hold Task 11.

## Not done

The Claude Code adapter, spikes S4 and S5, the plugin manifest, the marketplace entry and the README row, Task 11, and any merge or push. The second plan covers them. Its starting constraints are in the spec.
