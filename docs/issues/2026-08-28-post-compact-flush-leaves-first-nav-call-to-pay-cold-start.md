---
id: caa8bc1df0e8c0d8
kind: bug
status: open
title: 'BUG: workspace(post_compact) flushes LSP without prewarming, so the next navigation call pays cold start and can blow the 60s tool timeout — while its own hint promises no disruption'
owners:
- marius
tags:
- lsp
- cold-start
- post-compact
- references
- doc-vs-behavior
- timeout
---

## Summary

`workspace(post_compact=true)` flushes every LSP client and returns a hint saying
*"Clients restart automatically on the next navigation call. LSP clients restart
lazily — **no disruption to the session**."*

The restart is lazy, so the **first navigation call after the flush pays the whole
cold start** — on this 1697-file Rust crate that exceeded the 60s tool timeout and
the call died with no result. "No disruption" is the part that is wrong.

`workspace(action="activate")` does **not** have this problem: its documented step 3
prewarms LSP for the project's languages in the background. The flush path skips
that. The asymmetry is the finding.

## Symptom (Effect)

2026-08-28, first turn of a post-compaction session. Call sequence, in order:

1. `workspace(action="status", post_compact=true)` → `{"flushed": true, …}`, fast.
2. …six intervening non-LSP calls (`run_command`, `grep`, `read_file`)…
3. `references(symbol="impl MemoryStore/write", path="src/memory/mod.rs")`
   → `Tool 'references' timed out after 60s.`

No partial result, no "still indexing" signal — the tool timed out and the caller
is left to guess whether the symbol, the path, or the server was at fault. I
guessed the symbol name, and was wrong.

## Reproduction

Not yet reproduced deliberately. The natural setup is:

```
workspace(action="status", post_compact=true)      # flush, no prewarm
references(symbol=<any resolvable symbol>, path=<any file in a large crate>)
```

Timing is the whole variable — a small crate, or a rust-analyzer that another
session already warmed, will not show it. See *Resume* for what to capture.

## Environment

Branch `experiments` @ `894a5e26`, linux, codescout 0.15.0, release build,
rust-analyzer 1.97.1. Two other rust-analyzer instances and eight `codescout start`
processes were live on the host, so lock/CPU contention is a confound not excluded.

## Root cause

**Not established.** What IS established is that name resolution is not involved,
which is worth recording because it was my hypothesis and it looked well-supported.

The `references` seed path at `src/tools/symbol/references.rs:280-286` falls back to
`resolve_binding_by_position` when a name resolves to no document symbol, and that
fallback validates *every candidate occurrence* with `goto_definition`. Since
`symbols(name_path="impl MemoryStore/write")` returns **0 matches**, and `write`
occurs many times in that file, N LSP round-trips looked like a clean explanation.

**Refuted by probe** — see Evidence. The suspect name is fast once warm.

## Evidence

Three probes, same session, warm rust-analyzer, build idle (no lock contention):

| # | call | result |
|---|---|---|
| A | `references("MemoryStore/write", …)` — the *correct* name | **0 references**, fast, with the false-zero warning |
| B | `references("impl MemoryStore/write", …)` — the *timed-out* name | **34 references in 7 files**, fast |
| A′ | probe A again, minutes later | **34 references in 7 files** |

A → A′ is the same query returning 0 then 34, which isolates the variable to index
warmth. B being fast refutes the name-resolution hypothesis outright: the name I
blamed resolves fine.

Probe A's zero was **correctly guarded** — it carried *"the reference index may
still be warming after a reindex. Re-run, or corroborate with grep /
call_graph"*. That guard works. The timeout has no equivalent.

## Hypotheses tried

1. **Hypothesis:** the unresolvable name sent `references` into the
   per-occurrence `goto_definition` fallback, burning the timeout.
   **Test:** probe B, the same name, warm.
   **Verdict:** **rejected** — returns 34 refs immediately.

2. **Hypothesis:** OOM killed the call.
   **Test:** `journalctl -k` for the window. The only hits are an NVIDIA GPU
   `NV_ERR_NO_MEMORY` and an `OOM killer disabled`/`enabled` pair one second
   apart — the suspend/resume signature, not a kill.
   **Verdict:** rejected. (A `dmesg`-based count said `1` and was a false
   positive: `dmesg` is unreadable here, so the `||` fallback grepped a
   different source. Recorded because the wrong probe returned a *number*.)

3. **Hypothesis:** this is the zombie
   `docs/issues/2026-08-27-references-symbol-not-found-while-lsp-warms.md`
   recurring.
   **Test:** that file's re-open trigger is `references` returning
   **`symbol not found`** for a symbol `symbols(name=…)` resolves.
   **Verdict:** rejected — no `symbol not found` was ever returned. A timeout
   and a guarded zero are both explicitly outside that trigger, and that file
   says *"Do NOT re-file the refuted mechanism."* This is a separate bug.

## Fix

Not started. In preference order:

- **a. Prewarm after the flush.** `workspace(activate)` already does this
  (background, non-blocking, documented as step 3). `post_compact` flushes and
  stops. Making the flush path do what the activate path already does removes the
  cold window rather than documenting it.
- **b. Make the hint honest.** *"no disruption to the session"* is the sentence
  that cost the investigation. If (a) is not done, it should say the next
  navigation call may pay a cold start and can exceed the tool timeout.
- **c. Report, don't just die.** A timeout that returns "server still indexing,
  re-run" is a different experience from one that returns nothing. Compare probe
  A's false-zero, which is guarded and self-describing.

(a) and (b) are not exclusive; (a) alone would make (b) unnecessary.

## Tests added

None yet, and the honest note is that a *timing* assertion would be flaky. The
testable claim is structural, not temporal: **after a `post_compact` flush, a
prewarm is issued** — assert the call, not the latency.

## Workarounds

**Re-run the call.** The second attempt succeeds. If a navigation call is the
first thing you do after `workspace(post_compact=true)`, expect to pay for it
once.

Corroborate a suspicious zero with `grep` or
`call_graph(direction="callers")` before believing it — `references`' own warning
says so, and in probe A it was right.

## Resume

To pin the mechanism, capture **in one turn, before anything warms**:

1. `ps -o pid,etime -C rust-analyzer` immediately after `post_compact` — process
   age is what separates a genuine cold start from contention. The 2026-08-27
   probes on the sibling zombie were 19s old and did NOT reproduce, so age is the
   discriminator that file already relies on.
2. The `references` call, timed.
3. The same call again, timed.

Then read `workspace`'s `post_compact` branch and confirm whether it issues the
prewarm that the `activate` branch does. If it does not, (a) is a small change.

## References

- `docs/issues/2026-08-27-references-symbol-not-found-while-lsp-warms.md` — **zombie**, adjacent but NOT this: its trigger is `symbol not found`, and its cold-start mechanism is refuted
- `docs/issues/archive/2026-06-09-references-false-zero-stale-graph.md` — mitigated; the guarded false-zero probe A hit, working as designed
- `docs/issues/archive/2026-04-24-find-symbol-cold-start-hang.md` — fixed; the same 60s cold-start shape on `find_symbol`
- `docs/issues/archive/2026-07-10-lsp-shutdown-all-holds-clients-lock-across-await.md` — fixed; `post_compact` stalling navigation via a lock held during *shutdown*, which is the other half of this path
- `get_guide("workspace-state")` § *What `activate_project` does*, step 3 — the prewarm the flush path lacks

