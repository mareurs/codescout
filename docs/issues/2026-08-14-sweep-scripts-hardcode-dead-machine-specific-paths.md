---
id: f6dd06e3388e5465
kind: bug
status: open
title: 'BUG: both bm25 sweep scripts hardcode machine-specific absolute paths, and both point at a directory that no longer exists'
owners:
- marius
tags:
- scripts
- benchmark
- machine-specific
- dead-path
closed: null
opened: 2026-08-14
owner: marius
related:
- docs/issues/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md
severity: low
---

# BUG: both bm25 sweep scripts hardcode machine-specific absolute paths, and both point at a directory that no longer exists

## Summary

`scripts/sweep-bm25-boost.sh` and `scripts/sweep-bm25-cr1200.sh` both embed an
absolute path under `/home/marius/` as a committed default. Both paths resolve to
nothing as of 2026-08-14 — they point into the `code-explorer` registry root that
task #45 removed. The first script is the one `src/retrieval/config.rs` tells users
to run to re-derive `bm25_boost` for their own corpus, so the broken default sits
behind a documented pointer.

## Symptom (Effect)

Neither script fails at the path — they fail later, inside the benchmark, after
setup work. `sweep-bm25-boost.sh` invoked with no arguments passes a non-existent
project path to `scripts/run-tc-benchmark.py`; `sweep-bm25-cr1200.sh` has no
argument override at all, so it cannot run on any machine but the original one.

## Reproduction

```
ls -d /home/marius/work/claude/code-explorer
ls -d /home/marius/work/claude/code-explorer/.worktrees/retrieval-stack
```

Measured 2026-08-14 — both: `No such file or directory`.

Then, in the repo:

- `scripts/sweep-bm25-boost.sh:6` — `PROJECT_PATH="${2:-/home/marius/work/claude/code-explorer}"`
- `scripts/sweep-bm25-cr1200.sh:8` — `CORPUS=/home/marius/work/claude/code-explorer/.worktrees/retrieval-stack`

## Environment

codescout repo, `experiments` branch. Host-agnostic defect — the paths are wrong on
every machine including the one that wrote them, since #45 deleted the root.

## Root cause

Two separate causes that happen to land in the same two files:

1. **Machine-specific value in a committed file.** CLAUDE.md's rule is explicit:
   per-machine values belong outside every repo, because committing them makes the
   file read as false to anyone on a different host. These are benchmark
   *convenience* defaults, which is exactly how such values get committed without
   review friction.
2. **Nothing re-reads a script's default when the thing it points at is deleted.**
   Task #45 cleaned the `code-explorer` registry root and its three prunable
   worktrees. Neither script was in that task's blast radius, and no gate connects
   a removed directory to the string literals that named it.

`sweep-bm25-cr1200.sh` also pins a stack that no longer matches anything shipped
(`:43300` dense, `:8091` sparse, `openai` protocol) — that is *not* a bug in the
same sense, it is an ad-hoc experiment cell, but it means the script is unrunnable
for a second, independent reason.

## Evidence

`src/retrieval/config.rs` sends users to the first script by name, in the comment
justifying the `bm25_boost` default:

```
// ... both are observations, and users
// re-derive theirs with scripts/sweep-bm25-boost.sh. Inert while
// CODESCOUT_DISABLE_SPARSE is set.
```

So the documented path for "re-derive this tuning constant for your corpus" runs a
script whose zero-argument default cannot work.

## Hypotheses tried

1. **Hypothesis:** the `cr1200` script's `:8091`/`:43300` ports are the same
   endpoint drift as `docs/issues/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md`.
   **Test:** read the script header and compared against `docker-compose.yml`
   published ports. **Verdict:** rejected — it declares
   `CODESCOUT_EMBEDDER_PROTOCOL=openai` and a `43300` dense endpoint, which matches
   the `[embeddings].url` convention documented on `EmbeddingsSection::url`, not the
   compose stack. Its ports were deliberately different and were correctly left
   untouched when the endpoint-drift bug was fixed.

## Fix

Not implemented. Two candidate shapes, and the choice is a judgment call rather
than a mechanical fix:

- **Fail closed.** Drop both defaults; require the corpus path as an argument and
  exit with a usage message naming what is missing. Honest, and it cannot rot
  again. Costs the zero-argument convenience.
- **Default to the repo itself.** Benchmark codescout against codescout. Always
  valid, never machine-specific — but the tc-benchmark's scoring fixtures may be
  tuned to the `code-explorer` corpus, in which case a substituted corpus yields
  numbers that look fine and mean nothing. **Verify that before choosing this.**

Whichever is chosen, `sweep-bm25-cr1200.sh` may simply deserve deletion instead —
it is a single experiment cell pinned to a stack that is not the shipped one.
Check whether its results are cited anywhere first.

## Tests added

N/A — not fixed. A regression guard is feasible and worth adding with the fix: a
test asserting no file under `scripts/` contains a `/home/` literal would catch
this class permanently, and is the kind of guard that cannot silently stop working.

## Workarounds

Pass the corpus path explicitly: `scripts/sweep-bm25-boost.sh <binary> <project-path>`.
For `sweep-bm25-cr1200.sh`, edit `CORPUS` in place — it takes no argument.

## Resume

Decide fail-closed vs default-to-repo for `sweep-bm25-boost.sh`, and decide whether
`sweep-bm25-cr1200.sh` is deleted rather than fixed. Before choosing
default-to-repo, read `scripts/run-tc-benchmark.py`'s fixture/scoring set and
confirm whether its expected results are corpus-specific — if they are, a
substituted corpus produces meaningless scores that still look like scores, which
is worse than a script that refuses to run.

## References

- `scripts/sweep-bm25-boost.sh`
- `scripts/sweep-bm25-cr1200.sh`
- `src/retrieval/config.rs` — the `bm25_boost` comment that points at the first script
- `docs/issues/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md` — found while fixing this; same two files, different defect class
- Task #45 — removed the `code-explorer` registry root these paths name

