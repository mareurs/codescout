---
id: '86d0794657b1ab62'
kind: bug
status: fixed
title: 'BUG: both bm25 sweep scripts hardcode machine-specific absolute paths, and both point at a directory that no longer exists'
owners:
- marius
tags:
- scripts
- benchmark
- machine-specific
- dead-path
- live-verified-2026-08-14
closed: 2026-08-15
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

Shipped in `51283504`. The filing called this a machine-specific path; it is
that, but the sharper finding is *which* machine-specific path.

**`code-explorer` is codescout's own pre-rename name** — see
`docs/superpowers/plans/2026-03-04-rename-to-codescout.md`. The corpus default
was never pointing at a foreign project on the author's disk; it pointed at
**this repo**, under a name that stopped existing at the rename. That is why no
one noticed: the value looked like a personal convenience default, so nobody
re-read it, and the thing it named had been gone for months.

Confirmed by the benchmark itself: `run-tc-benchmark.py`'s `expected` lists name
codescout's own tree — `src/tools/core/types.rs`, `src/lsp/client.rs`,
`src/embed/index.rs`, `docs/FEATURES.md`, `docs/PROGRESSIVE_DISCOVERABILITY.md`.
Any other corpus scores meaningless numbers. So the correct default is not
"nothing, make the user pass it" but **the repo root, derived**:

- `sweep-bm25-boost.sh`: `PROJECT_PATH="${2:-$(cd "$(dirname "$0")/.." && pwd)}"`
- `sweep-bm25-cr1200.sh`: `CORPUS="${CORPUS:-$PWD}"` (after the existing `cd` to
  the repo root)

The cr1200 endpoint pins are now env-overridable
(`${CODESCOUT_EMBEDDER_URL:-...}`) with a comment stating what the filing
observed: the cell measures CodeRankEmbed at chunk 1200 and needs an endpoint
actually serving that model, which is **not** the stack `docker-compose.yml`
publishes. Left as documented ad-hoc values rather than retargeted, because
changing the endpoint would change what the experiment measures.
## Tests added

`tests/committed_paths.rs` — the gate whose absence is root cause 2. No
committed script may name a path under a personal home directory.

- `no_committed_script_hardcodes_a_personal_home_path` — walks `scripts/`,
  reports every `file:line`, and names the derive-it-instead fix in the failure
  message.
- `the_home_path_scan_discriminates` — pins `account_after`'s parsing and the
  `UNIVERSAL_ACCOUNTS` exemption, so a scanner that returned `None` for
  everything cannot leave the gate vacuously green.
- `the_scan_actually_reads_files` — guards against a wrong `CARGO_MANIFEST_DIR`
  join making the walk find nothing. Same false-green shape that let this bug
  ship.

**Proven able to fail before being trusted.** A planted probe
(`scripts/__mutation_probe.sh` with `CORPUS=/home/someone/work/thing`) produced
`scripts/__mutation_probe.sh:2 — /home/someone`; probe removed after.

Scope is `scripts/` on purpose. `docs/` home paths are *records* — measured
output, quoted sessions, archived reports — and rewriting them would falsify
history. `.github/workflows/` is exempt for a different reason: `/home/runner`
is GitHub's path on every runner alive, so it is not machine-specific;
`UNIVERSAL_ACCOUNTS` carries `runner` for the same reason.

Verification: `bash -n` clean on both scripts; the boost default resolves to
`/home/marius/work/claude/codescout` (the repo root) on this host.
## Workarounds

Pass the corpus path explicitly: `scripts/sweep-bm25-boost.sh <binary> <project-path>`.
For `sweep-bm25-cr1200.sh`, edit `CORPUS` in place — it takes no argument.

## Resume

Closed. One residual, deliberately not fixed: `sweep-bm25-cr1200.sh` still needs
a CodeRankEmbed endpoint that nothing in this repo starts. That is an ad-hoc
experiment cell, not a defect — it is now documented in the script rather than
silently pinned.
## References

- `scripts/sweep-bm25-boost.sh`
- `scripts/sweep-bm25-cr1200.sh`
- `src/retrieval/config.rs` — the `bm25_boost` comment that points at the first script
- `docs/issues/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md` — found while fixing this; same two files, different defect class
- Task #45 — removed the `code-explorer` registry root these paths name
