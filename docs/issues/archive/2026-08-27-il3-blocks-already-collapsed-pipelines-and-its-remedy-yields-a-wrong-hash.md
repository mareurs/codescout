---
id: 5b4679128f978d8b
kind: bug
status: fixed
title: IL-3 blocks pipelines that already collapsed to one line, and its prescribed buffer remedy yields a silently wrong patch-id
tags:
- run_command
- iron-laws
- il3
- false-positive
- buffers
---

# IL-3 blocks pipelines that already collapsed to one line, misclassifies field-selectors as trimmers, and routes the caller to a truncated buffer that yields a silently WRONG hash

**Found:** 2026-08-27, from `claude-plugins`, while recording a fix's `patch-id` — the
exact workflow `get_guide("tracker-conventions")` mandates.
**Affects:** the IL-3 pipe guard in `run_command`.
**Related:** `docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md`
— finding 4 below is that bug's failure mode, composed with this one and made worse.

## Summary

The IL-3 guard blocks a pipeline if its head is an "unbounded producer" and **any** stage
matches a trimmer — regardless of what intermediate stages already collapsed, regardless
of the pipeline's actual output size, and regardless of whether the final stage is an
explicitly-allowed aggregator.

Its own suggested remedy ("rerun bare and query the `@cmd_*` buffer") then routes the
caller to a **truncated** buffer. For a selection task that yields a partial answer. For a
**hashing** task it yields a confident, syntactically perfect, **wrong** answer.

## Symptom — five measured cases

| command | output size | verdict |
|---|---|---|
| `git show X \| git patch-id --stable` | 1 line | **allowed** |
| `git show X \| git patch-id --stable \| cut -d' ' -f1` | 1 line | **BLOCKED** |
| `git show X \| cut -d' ' -f1 \| wc -l` | 1 line | **BLOCKED** — final stage is `wc`, an explicitly-allowed aggregator |
| `git rev-parse HEAD \| head -1` | 1 line (40 chars) | **BLOCKED** |
| `git patch-id --stable < file \| awk '{print $1}'` | 1 line | **BLOCKED** |

Row 1 vs row 2 is the core defect: `git patch-id` collapses an arbitrary diff to exactly
one line, the guard *allows* that, and then adding a `cut` on that single line blocks the
whole thing. Row 3 shows no amount of downstream collapsing rescues it — the scan finds a
trimmer anywhere and stops thinking.

## Finding 1 — the trimmer scan ignores what the pipeline already collapsed

A pipeline's purpose is reduction. The guard evaluates head-vs-trimmer-set over the whole
chain, so it cannot see that stage 2 already reduced 912 lines to 1. `wc -l` as the final
stage — a member of the guard's own allowed-aggregator list — does not help.

**Suggested fix:** stop scanning once a *collapsing* stage is seen. If any stage is a known
aggregator (`wc`, `grep -c`, `sort -u | wc`, `sha256sum`, `md5sum`, `git patch-id`, …), no
downstream stage can re-expand the output, so downstream trimmers are irrelevant to the
context-flooding harm IL-3 exists to prevent.

## Finding 2 — `cut` is not a trimmer

`head`, `tail`, `sort` change **which records** you see — that is the information loss IL-3
guards against. `cut` selects **fields within every record**: it is 1:1 on lines and cannot
hide a record. The guard's own word for the class, "log-trimmer", does not describe it.

Same argument applies to `tr` and to `awk '{print $N}'` (note `awk` is already on the
guard's *bounded-LHS* list, yet appears to count as a trimmer on the RHS — row 5).

## Finding 3 — every one-line `git` plumbing subcommand is classified "unbounded"

The documented limiter vocabulary — `-n`, `--max-count`, `-3`, `--show-current`,
`--porcelain/--short`, `--stat` — is `git log` / `git status` grammar. Subcommands that
emit exactly one line **by construction** have no such flag, so they are permanently
unbounded:

`git patch-id`, `git rev-parse`, `git merge-base`, `git config --get`, `git symbolic-ref`,
`git describe`, `git hash-object`.

`git rev-parse HEAD | head -1` being blocked as an unbounded producer piped to a
log-trimmer is the clearest statement of the problem.

**Suggested fix:** an allowlist of single-line git plumbing subcommands, checked before the
limiter-flag heuristic.

## Finding 4 — the prescribed remedy silently produces a WRONG hash

This is the serious one. Following the error's own instruction exactly:

```
git show ba2d214 | wc -l
→ { "stdout": "912", "unfiltered_output": "@cmd_4277ec40",
    "unfiltered_output_lines": 912, "unfiltered_truncated": true }

wc -l < @cmd_4277ec40                        → 181        # buffer holds 181 of 912 lines
git patch-id --stable < @cmd_4277ec40        → 42e7b21b32290461dde1161869613f6f3d972c88
git patch-id --stable < <full 912-line diff> → 2e9c082fb34c06d51c9be686e3aa019938cf3d56
```

The buffer-derived value is a **syntactically perfect, wrong** 40-hex hash, emitted with
exit 0, paired with the **correct** commit id — so it looks entirely trustworthy.

For a `grep`, a truncated buffer gives a partial answer. For a **hash**, it gives a
confident wrong one: there is no partial sha, and no downstream check can catch it.

And the stakes are specific, not hypothetical. `get_guide("tracker-conventions")` requires
recording a `patch-id` beside a fix SHA when archiving a bug, precisely because it is
*"invariant under rebase and cherry-pick, so it still finds the change after the SHA dies."*
That guide cites a measurement that **10 of 63** archived bug files had already lost their
fix pointer. So this chain — block the correct command, recommend the buffer, hash the
truncated buffer — silently corrupts the one identifier the project designed to be
permanent, in the exact workflow that mandates it.

**Suggested fix:** never propose an `@cmd_*` buffer as a substitute for the original output
when that buffer is flagged `unfiltered_truncated: true`. Prefer recommending the
file-redirect pattern the project's own `tracker-conventions` guide already documents:

```
git show <sha> > /tmp/x.patch
git patch-id --stable < /tmp/x.patch
```

## Finding 5 — the diagnostic misquotes multi-statement scripts

The originating command was a five-line script containing a `for` loop. The error rendered
**the entire script** as the thing "piped to a log-trimmer", and blocked all of it —
including a trailing `git branch --contains <sha>` containing no pipe at all. The offending
pipe was inside the loop body; nothing in the message localizes it.

**Suggested fix:** split on `;` / newline / `&&` and report the specific offending
pipeline.

## Cross-cutting: the guard is strictest where it understands least

Read alongside the sibling bug on truncated-buffer reporting, the two behaviours are
inverted:

| producer | guard's response |
|---|---|
| **recognized** (`git show`) | hard block, even when output is one line |
| **unrecognized** (`./tests/run-all.sh`) | allowed, silently truncated tee |

The case it can classify gets the hard stop. The case it cannot classify — a local script
that may emit anything — gets the silent lossy path. That is backwards from a safety
standpoint, and both halves were hit by the same session within one hour.

## Reproduction

```
git show <any-sha> | git patch-id --stable              # allowed
git show <any-sha> | git patch-id --stable | cut -d' ' -f1   # BLOCKED
git show <any-sha> | cut -d' ' -f1 | wc -l              # BLOCKED
git rev-parse HEAD | head -1                            # BLOCKED
```

## Environment

- Claude Code 2.1.247, codescout `experiments`, measured 2026-08-27 from
  `/home/marius/work/claude/claude-plugins`.

## Fix

Shipped on `experiments` in **`18f8f9d1`** — patch-id **`16a9abc0d985d8f8ed1886a778e4a84166190115`**
(the patch-id is the durable pointer; the SHA orphans on the next rebase).
Probe promoted alongside in `e5ab9e90` as `scripts/probe_il3_refusals.py`.
Prompt surfaces followed in **`5d3f8ebe`** — patch-id
**`62f198ccab11c4b76a1ac3cdafe6338263308bed`**: `refusal_predicate` and
`iron-laws-detail` both stated the pre-fix rule to the agent, and a refused caller
reads the predicate to predict the next refusal. That commit also corrects a claim
that was false *before* this fix — the guide listed `| wc` as a log-trimmer and
called `wc` "forbidden as a trimmer on the RIGHT of a pipe", which
`il3_allows_cargo_pipe_wc` had been contradicting the whole time. Nothing read both.

**Measured before designing anything** (R-117 — a fix that names a population
asserts it is non-empty). Across **703 IL-3 refusals in 37 `usage.db` files**:

| bucket | n | % |
|---|---:|---:|
| correctly blocked | 581 | 82.6% |
| already retired by earlier fixes | 103 | 14.7% |
| **fixed here** | **19** | **2.7%** |

The frequency case is modest; the severity case is not, and they are separate
arguments. 12 of the 19 are the `git patch-id` workflow itself.

### What shipped, per finding

- **Finding 1 — shipped, in a STRONGER form than proposed.** The file's
  suggestion was *stop scanning once a collapsing stage is seen*. Running a
  classifier against the corpus showed that rule leaves trimmers **upstream**
  of the collapser counting — so it does **not** move this file's own row 3
  (`git show X | cut -f1 | wc -l`), the example it cites as evidence. What
  shipped instead: *a collapsing stage anywhere downstream bounds the whole
  pipeline*. That also closes an inconsistency the file did not name —
  `git log | grep -c fix` was allowed while `git log | grep fix | wc -l`, the
  identical single number reaching the agent, was refused.
- **Finding 2 — shipped for `cut` and `tr` only.** Both are 1:1 on records and
  cannot hide one. **`awk '{print $N}'` was NOT exempted**, contrary to the
  suggestion: `awk` is a general-purpose language and `awk 'NR<10'` selects
  records, so exempting it would need the guard to parse an embedded language.
  Classified on capability, not on typical use. `sed` and `sort` stay for the
  same reason (`sed -n '1,10p'`, `sort -u`), each pinned by a control test.
- **Finding 3 — shipped.** `git_subcommand_is_single_line` allowlists
  `rev-parse`, `patch-id`, `merge-base`, `symbolic-ref`, `describe`,
  `hash-object`, `config`, checked before the limiter heuristic.
  `rev-parse --all` and `config --list` are excluded on those flags rather
  than dropped from the list — both pinned by control tests.
- **Finding 4 — shipped as a WARNING, not as suppression.** The error now names
  `unfiltered_truncated`, states plainly that a hash of a truncated buffer is a
  valid-looking wrong digest, and offers the file-redirect pattern for
  whole-input work. It does **not** withhold the `@cmd_*` recommendation when
  the buffer would be truncated, because at block time the command has not run
  and the guard cannot know. Making the truncation observable at read time is
  the sibling bug's Option C and is still open there.
- **Finding 5 — shipped.** The message quotes the offending `;`/`&&` segment
  rather than the whole command.

### Incidental finding — the mirror hook is dormant

`detect_il3_violation`'s doc comment instructed the reader to keep
`codescout-companion/hooks/il3-deny-hook.sh` in sync. It has not run for some
time: no `hooks.json` PreToolUse matcher targets `run_command`, and
`cargo --version; ls docs | head -3` — which the hook's non-segment-splitting
logic would refuse — is allowed end to end. Two independent signals, one
behavioural and one a registration inventory. The doc comment now records the
measurement; the dead file is tracked separately in
`docs/issues/archive/2026-08-27-il3-deny-hook-is-dormant-but-documented-as-a-live-mirror.md`.
(That file's own framing was later **corrected**: the hook's dormancy is deliberate and
documented, and the real defect was that this commit changed the server's semantics
without syncing the parked mirror — fixed in `claude-plugins:88f1e29`.)

### Tests

14 added. Each was verified to **fail** with its change reverted — a three-way
revert produced exactly 8 failures, each attributable to one change — while all
four `still_blocks_*` controls stayed green under that same revert. That pairing
is what makes the loosening a measured line rather than a claim. All 54
pre-existing IL-3 invariants pass unchanged, including
`il3_blocks_git_pipe_head_still`.

Full gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
`cargo test` — **4581 passed, 0 failed**. Live-verified after `cargo rb` + `/mcp`.

## Workarounds (for consumers, today)

- Redirect to a file, then transform the file. This is already the documented pattern in
  `get_guide("tracker-conventions")` § patch-id.
- Never feed an `@cmd_*` buffer to a hash, checksum, or any whole-input function without
  first checking `unfiltered_truncated` and comparing `wc -l` against
  `unfiltered_output_lines`.

## References

- `docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md` —
  sibling bug; finding 4 is its failure mode applied to a hash function
- `claude-plugins:roster-audit-session-log:F-14` — the truncated-buffer incident
- `get_guide("tracker-conventions")` § patch-id — the workflow this breaks
