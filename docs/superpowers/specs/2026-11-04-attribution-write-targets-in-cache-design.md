---
id: '5916cc3ea41eff3a'
kind: spec
status: active
title: Store each operation's write targets in the history cache, output kept identical
tags:
- attribution
- file-provenance
- performance
---

# Store each operation's write targets in the history cache, output kept identical

Date: 2026-11-04. Size: a small change. Bug: `20927ccfe293c34e`. Queue entry:
`resume-context-injection-remaining-work:CIW-10`. Predecessor:
[`2026-11-01-policy-loop-speedup-design.md`](2026-11-01-policy-loop-speedup-design.md), which listed this as
approach 2 and deferred it.

## Intent and why

The red-attribution hint runs `scan_provenance` inside a 30 s ceiling, and the scan blocks the red's
response. The bug's reopen trigger fired on 2026-11-04: a real red took 19,917 ms against a 15,000 ms
trigger. A warm scan is about 13 s, of which about 5.81 s is `write_targets` re-deriving every
operation's targets from its input (phase timers in the bug file; an estimate, not a measurement).
Those targets are a function of the operation alone, except one branch, so they can be computed once
per history file and stored with the decoded operations. The operator chose this option on 2026-11-04.

## Goals

- A warm scan is measurably faster on the real corpus. Target, from the phase timers and not
  measured: up to about 5.8 s of 13 s.
- `scan_provenance` returns the same `ProvenanceScan` (owners, evidence, gaps, streams) as before for
  every input.

## Non-goals and anti-goals

- No lean "owners only" path and no floor on operations; the CLI needs the full answer.
- No change to the 30 s ceiling.
- Anti-goal: a faster scan that differs from the old one on any real input, even in gaps.
- Anti-goal: an edit to the ownership policy in `file-provenance.py` emptying the cache. That was the
  cost the predecessor recorded against this approach, and the module split below exists to avoid it.

## Constraints

- The engine is embedded in the Rust binary. A new module needs an `EMBEDDED_SCRIPTS` entry in
  `src/tools/run_command/attribution.rs` (7 → 8). `the_materialized_set_is_closed_under_its_own_local_imports`
  fails if it is missing.
- The cache key covers `_CODE_FILES` in `scripts/history_cache.py`. Any module whose bytes decide a
  stored value must be in that list.
- `write_targets`' doc-tool branch reads the live catalog (`catalog_paths()`) to turn an artifact id into a
  path. That part is not a function of the operation and must not be stored.

## Approaches

1. **Split the pure extraction into its own module, store its result (chosen).** Only that module joins
   the cache key, so policy edits keep the cache.
2. Store targets computed in place, with `file-provenance.py` added to the cache key. Simplest, but every
   edit to the policy file empties all entries (2,355 here; a cold refill took about five 30 s calls).
3. A second, separate targets cache keyed on its own digest. Avoids re-decoding after a target-code change,
   but adds a second cache with its own invalidation rules for a saving that approach 1 already gets.

## The design

1. New module `scripts/write_targets.py`: the write-tool name sets and the pure extraction functions moved
   out of `file-provenance.py` unchanged in behaviour (`BASH_WRITE_PATTERNS`, `relocation_targets`,
   `python_write_targets`, `patch_write_targets`, and the target-name constants they need). It exposes
   `static_targets(name, inp)`, which yields everything `write_targets` yields today EXCEPT the
   catalog-id lookups, in the same order. `file-provenance.py` imports from it.
2. `Operation` (in `scripts/provenance_records.py`) gains `targets: tuple[str, ...] | None = None`.
3. A wrapper fills `targets` with `tuple(static_targets(op.name, op.input))` for every operation of a decoded
   batch. `scan_provenance` applies it inside the cached reader, so stored batches carry targets.
   `_CODE_FILES` gains `write_targets.py`.
4. `write_targets(name, inp, root)` keeps its signature and output: the static part, then the live catalog
   lookups for the doc tool. `scan_provenance` uses the stored static part when present
   (`op.targets is not None`), recomputes it when absent, and always adds the live catalog part after it,
   so the order of targets is unchanged.

## Proof the output is identical (what the builder must run)

- Unit tests with the old code as an oracle: for a table of real tool inputs (every branch, including a
  doc move with both `new_rel_path` and an `id`), `list(old_write_targets(...)) ==
  list(static_targets(...)) + list(live catalog part)`, in order.
- A cached batch carries targets, and a scan over a cached batch equals a scan over a fresh decode.
- The cache key changes when `write_targets.py` changes, and does not change when `file-provenance.py`
  changes.
- On the real corpus, frozen to files untouched for 15 minutes, in ONE process: old module, new module,
  old, new. All four give one digest (owners, evidence, gaps, streams).
- Mutants that must fail a test: a static part that drops the catalog lookups without the live part
  adding them; targets stored under a key that omits `write_targets.py`; the live part placed before the
  static part.
- Warm wall time, old and new, a few runs each on the same load, through `attribute-red.py`.
- `./scripts/gate.sh`.

## Left to the builder

- The wrapper's exact place (inside `caching_reader`'s wrapped reader, or a reader decorator), as long
  as stored batches carry targets and an uncached read gives the same result.
- Whether `write_targets` in `file-provenance.py` becomes a thin composition or stays a re-export.
- If the measured warm saving is under 15%, stop and report.

## Risks, with what I have not shown

- The 5.81 s figure is from phase timers, not from a before/after measurement.
- The first scan after this change misses every entry once (the cache key changes), so the next few reds
  may be silent while the cache refills, as after any reader change.
- Under heavy load the whole scan grows, so this alone may not keep every red under 15 s.
