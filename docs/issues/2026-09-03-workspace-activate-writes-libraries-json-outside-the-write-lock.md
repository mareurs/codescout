---
id: '34be359cd8087698'
kind: bug
status: open
title: workspace(activate) writes libraries.json while is_write reports false, so the write lock never fires
tags:
- cluster/guard-narrower-than-its-name
- write-guard
- concurrency
- shared-checkout
closed: null
opened: 2026-09-03
owner: marius
related: []
severity: medium
unverified: No race observed; the mechanism is read from the code, not measured at runtime. Fix shape is undecided.
---

# BUG: `workspace(action="activate")` writes `libraries.json` while `is_write` reports false, so the write lock never fires

## Summary
`Workspace` does not override `Tool::is_write`, so every `workspace` call reports `false`
and never acquires the cross-process write lock. But `action="activate"` calls
`auto_register_deps`, which persists `.codescout/libraries.json` to disk. On a shared
checkout two sessions activating concurrently can interleave that write with no guard.

Found 2026-09-03 while classifying tools for MCP tool annotations (SM-1): the truthful
annotation (`readOnlyHint` absent — a writer) disagrees with `is_write` (`false` — a
reader), and only one of the two can be right about the disk.

## Symptom (Effect)
No error, and no observable symptom in the common case. The write is small, fast and
usually idempotent, so a lost update looks like "the registry didn't pick up that
dependency" rather than like a race.

## Reproduction
Not yet reproduced as a race — filed on the mechanism, which is read directly from the
code (below). To reproduce the *mechanism*:

```
git rev-parse HEAD          # 88311708-dirty at filing
```

`workspace(action="activate", path=<a project with undeclared deps>)` writes
`.codescout/libraries.json`; `Workspace::is_write` is never called with a true result
because the override does not exist.

## Environment
codescout `experiments`, all platforms. Matters most on this machine, where several agent
sessions routinely share one checkout (`CLAUDE.md` § *Reaching a Peer Session*).

## Root cause
`impl Tool for Workspace` (`src/tools/config/mod.rs:12`) supplies no `is_write` override,
so it inherits the trait default `false` (`src/tools/core/types.rs:833-835`).

`action="activate"` reaches `auto_register_deps`, which ends in
`project.library_registry.save(&registry_path)` — a real filesystem write
(`src/library/auto_register.rs:64-65`). The pinning test at `src/library/auto_register.rs`
asserts the file lands in the pinned workspace, so the write is deliberate and covered;
what is missing is the *lock*.

`is_write` is consumed at `src/server.rs:1048-1055` and `:534-539` to decide whether to
take the cross-process write lock and to upgrade pinned-workspace residency. A `false`
there means neither happens.

inferred from the sites above — **not measured as an observed race.** No interleaved
corruption has been seen; this is a missing guard, not a reported failure.

## Evidence

### The annotation pass is what surfaced it
Classifying all 21 tools for `readOnlyHint` forced the question "does this tool modify its
environment?" for each one. `workspace` is the only tool where the honest answer to that
question and the value of `is_write` disagree in the direction that matters — `is_write`
says reader, the disk says writer. `run_command` disagrees too but in a known and
deliberate way (shell writes were left outside the lock on purpose).

The annotation shipped truthfully (`src/tools/config/mod.rs`, `additive_closed()`), with a
comment pointing here rather than silently matching the annotation to `is_write`.

## Hypotheses tried

1. **Hypothesis:** the write is incidental bookkeeping and does not warrant the lock.
   **Test:** read `auto_register_deps` and its pinning test.
   **Verdict:** deferred — it is a genuine file write inside the user's project, but it is
   additive and idempotent, so the cost of a race is low. Whether that justifies leaving it
   unguarded is a judgement the code does not record either way.

## Fix
Not attempted. Two candidate shapes, and the choice is not obvious:

- **Override `is_write`** to return true for `action="activate"`. Correct and one line, but
  it makes every activation take the cross-process write lock, and activation happens on
  nearly every session start — a contention cost paid constantly for a rare race.
- **Move the registry write behind its own file lock**, leaving `is_write` false. Narrower
  blast radius, more code.

This is the same class as `docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`
(`93caba562c06a258`), which chose the first shape for the librarian adapter. That precedent
is not automatically right here, because the librarian actions are user-initiated writes
and this one is a side effect of a routine navigation call.

## Tests added
None — filed, not fixed. `server::tests::annotations_agree_with_is_write` deliberately does
**not** assert the converse direction (`is_write == false` implies read-only), precisely so
it does not encode this gap as a rule.

## Workarounds
None needed in practice today. Sessions sharing a checkout should avoid activating the same
project simultaneously if the registry is being rewritten, but the write is additive and
idempotent so the realistic damage is a dropped dependency entry, recoverable by
re-activating.

## Resume
Decide between the two fix shapes above before writing code — the question is whether
activation is frequent enough that an unconditional write lock is worse than the race it
prevents. Measure how often `workspace(activate)` actually mutates `libraries.json`
(as opposed to writing an unchanged file) from `usage.db` first; if the mutating case is
rare, a conditional `is_write` keyed on "the registry actually changed" is a third option
neither bullet covers.

## References
- `src/tools/config/mod.rs:12` (`impl Tool for Workspace`), and its `annotations()` override
- `src/library/auto_register.rs:64-65` (the write)
- `src/tools/core/types.rs:833-835` (`is_write` default)
- `src/server.rs:1048-1055`, `:534-539` (the consumers)
- `docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md`
- `docs/trackers/resume-tool-surface-structural-mechanisms.md` — SM-1, the pass that found it
