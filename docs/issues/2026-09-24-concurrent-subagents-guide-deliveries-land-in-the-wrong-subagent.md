---
id: e76556484627a41a
kind: bug
status: open
title: 'BUG: concurrent subagents'' guide deliveries land in the wrong subagent — adoption swaps one shared ledger slot mid-call'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-09-24
severity: medium
---

# BUG: concurrent subagents' guide deliveries land in the wrong subagent — adoption swaps one shared ledger slot, and a slow call reads it after a sibling swapped it again

## Summary

With parallel subagents on one codescout server, a guide's first delivery is sent in **one subagent's response** and recorded against **another subagent's ledger**. The subagent that got the extra copy has it several times over. The subagent whose ledger was stamped never receives the guide at all, and nothing reports this. Each subagent's principal stamp is correct; the fault is that adoption turns that per-call identity into "whichever ledger currently occupies the one process-wide slot", and nothing keeps the slot fixed for the rest of the call.

## Symptom (Effect)

Four read-only verifier subagents were dispatched in parallel from session `ebf651ec-5ab7-42d9-a526-dcf9758692e1` at about 2026-09-24T11:41Z. They are called A–D here; agent ids are `aad0dcad0ae420b06`, `aec63cebff90c5c1d`, `a77fae2662f3ad4a2` and `a55e2cb3dd28a0e0f`. Each subagent's on-disk ledger (`~/.local/state/codescout/guide_hints/<sid>_<agent>.json`) records `symbol-navigation` **once**. Counting `auto-injected get_guide('symbol-navigation')` blocks per transcript line in each subagent's own transcript gives:

| ledger stamped (owner) | injection actually delivered to |
|---|---|
| 11:41:47 (A) | A at 11:41:48 |
| 11:41:51 (B) | B at 11:41:51 |
| 11:42:27 (**C**) | **B** at 11:42:27 |
| 11:44:06 (**D**) | **B** at 11:44:06 |

C's and D's transcripts contain **zero** `symbol-navigation` injections, yet their ledgers say it was delivered, so neither will receive it for the rest of the session. B received it three times, each copy saying "first call this session that triggers the topic". B noticed and reported the repetition, and that report is how this was found.

## Reproduction

Dispatch several subagents in parallel whose first calls include a slow LSP-backed tool (`symbols`, `references`). Then, for each subagent, compare its ledger file against the injections in its own transcript. A cheaper deterministic repro for a test: two principals, where call X is adopted and its tool body blocks on a barrier; call Y is adopted meanwhile; then X is released. X's guide delivery reads and stamps Y's ledger.

## Environment

`experiments` at `436a8ff6`; live MCP binary built after `a126bf48` (re-arm fix) and `971ed73f` (re-adopt restores the on-disk ledger). The companion `PreToolUse` principal stamp was present: each subagent has its own ledger file, which only exists when calls arrive stamped.

## Root cause

- `src/server.rs:679`: every `ToolContext` gets `guide_hints_emitted: self.guide_hints_emitted.clone()`. That is an `Arc` clone, so every in-flight call shares **one** `Mutex<GuideLedger>` slot.
- `adopt_request_conversation` (`src/server.rs:1204`, called at `:1386`) does not pick a ledger for the call. It **swaps the slot's contents**: it parks the outgoing ledger, then `*live = restored` or `live.adopt(&target)`. After that it releases the lock.
- The tool body and its guide delivery run later (`:1446`, `tool.call_content(input, &ctx)`). They re-lock the same slot (`ctx.guide_hints_emitted.lock()`, e.g. `src/tools/guide.rs`; post-phase emitters via `run_post`).
- Between those two points, another principal's call can swap the slot. The slow call then reads and **writes** the other principal's ledger. It injects into its own response and stamps someone else as served.

This is `cluster/transient-shared-state-lies-to-readers`: the slot's contents are correct for *some* principal at every instant, just not for the reader holding it.

## Evidence

The ledger files and per-transcript injection counts are above. Timestamps line up to the second, including the two misdirected deliveries. Measured from the subagent task transcripts under this session's `tasks/` directory, and not reproduced a second time.

## Hypotheses tried

1. **The hook stamped B's calls with C's/D's agent ids.** Not ruled out by the timings alone, but the server code needs no hook fault to produce exactly this table, and the race is structural. A per-call log of the stamped principal next to the adopted slot key would separate the two.
2. **B's own ledger failed to deduplicate.** Rejected: B's ledger holds one stamp, from its own first delivery.

## Fix

Not started. Direction: resolve the ledger **once**, at adoption, and carry that principal's ledger (or its key) in the `ToolContext` for the whole call, instead of an `Arc` to the process-wide slot. Parked ledgers already exist per principal, so a map of `principal -> Arc<Mutex<GuideLedger>>` looked up at adoption would remove the shared read-after-swap window. Persisting and parking then act on that per-principal entry.

## Tests added

None yet. The regression test needs two principals interleaved with a barrier inside the first call's tool body. A sequential test cannot reach this path. That is probably why the existing `guide_hint_tests`, which all run calls one after another, are green.

## Workarounds

None for affected subagents. A subagent that suspects it was starved can call `get_guide(<topic>)` explicitly: the body is never withheld on an explicit fetch.

## Resume

Write the barrier test first and watch it fail at HEAD, then scope the ledger per call. Check whether section-level (`librarian#…`) deliveries share the same path; they use the same ledger.

## References

- `docs/issues/2026-09-24-residual-in-session-subagent-guide-starvation.md`: the starvation residual. That one is about parent vs subagent through a shared `session_id`; this one is about sibling vs sibling under correct per-principal stamps.
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`: the design this defect undermines.
- `a126bf48`, `971ed73f`: today's ledger fixes. Both are correct for sequential calls; this is the concurrent case.
- Workflow context: `deep-agent-workflow-observations:DWF-7`.
