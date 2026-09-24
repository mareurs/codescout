---
id: c161cc27ddff5672
kind: bug
status: fixed
title: 'BUG: concurrent subagents'' guide deliveries land in the wrong subagent — adoption swaps one shared ledger slot mid-call'
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-09-24
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

**FIXED in `69e89228` (2026-09-24). Adoption now moves ledger handles instead of swapping contents, and each call keeps the handle it resolved.**

- The slot holds a handle: `live_ledger: Arc<Mutex<LedgerHandle>>`, where `LedgerHandle = Arc<Mutex<GuideLedger>>`. `adopt_request_conversation` takes the slot lock for the whole decision. It moves the target's handle out of `parked_ledgers` (or builds one exactly as before: clone the outgoing ledger, then `GuideLedger::adopt`), files the outgoing handle under its key, and **returns the resolved handle**.
- `call_tool_inner` uses that handle for `set_session_start_source`, for `poll_guide_rearm` (now a parameter), and for the `ToolContext`. `build_context` takes the handle as a parameter, so nothing can take a snapshot of the slot after a sibling's adoption.
- **One refinement over this file's original direction** ("a map of `principal -> Arc<Mutex<GuideLedger>>`"): the map must still **never hold the live ledger**. `poll_rendezvous` rekeys the live ledger in place on `/clear`, so filing it under its old key would later restore the wrong conversation's history (`bug-fix-session-log:W-145`). The live handle therefore stays out of the map, as it did before.

The live MCP binary picks this up only after a release rebuild (`./scripts/rb.sh`) and `/mcp` reconnect. Until then, running sessions still have the old behaviour.

## Tests added

- `server::guide_hint_tests::concurrent_principals_each_receive_their_own_first_call_guide`: two real overlapping `call_tool_inner` calls. X runs `sleep 0.8; echo x`, Y's call starts 250 ms in, and both pass `effect: "read"` so the write guard cannot serialize them. **Observed RED before the fix**: X's first call got no opener, and Y's control assertion passed. Green after.
- `server::guide_hint_tests::a_parent_keeps_its_one_shot_notices_across_a_subagent_hop`: covers the parked path. It closes a coverage gap the pre-fix code shared: dropping `parked.insert` SURVIVED all 57 existing `guide_hint_tests`, because topics write through to disk and re-adoption silently reloads them from the principal's file. Only the in-memory `notices` need the parked handle. The mutation is KILLED by this test.

**Mutation record** (`scripts/mutation-probe.sh`, isolated worktree, one mutation per site):

| site | mutation | verdict | reading |
|---|---|---|---|
| `call_tool_inner` | drop `ctx.guide_hints_emitted = ledger` | SURVIVED | unreachable: the adoption-to-`build_context` stretch has no `.await`, so only another worker thread can adopt inside it, and no test can place it there. **Repaired by removing the window** (`build_context` takes the handle) rather than keeping an untestable line. |
| `adopt_request_conversation` | drop `parked.insert(key, outgoing)` | KILLED | by the notices test above |
| `poll_guide_rearm` | re-read the slot instead of `ledger` | SURVIVED | same no-await window. Guarded by the parameter's shape, **not by a test**; stated as a limit. |

## Workarounds

None for affected subagents. A subagent that suspects it was starved can call `get_guide(<topic>)` explicitly: the body is never withheld on an explicit fetch.

## Resume

Nothing left. **Live check passed 2026-09-24 on the rebuilt binary.** `./scripts/rb.sh` built the release binary at 14:47:13Z from HEAD `a8835d06`, which contains `69e89228`, and was followed by an `/mcp` reconnect. Four read-only subagents were then dispatched in parallel, each with three LSP-backed first calls.

| subagent | ledger stamp → delivery in its **own** transcript (`project-activation-bootstrap`) | same for `symbol-navigation` |
|---|---|---|
| `afe2fc6cf4ed9dfd7` | 14:49:16.284 → 16.576 | 16.983 → 17.273 |
| `a12d8bd42cc779c7c` | 17.228 → 17.543 | 17.843 → 18.140 |
| `a4771bc3e6c7100bc` | 17.541 → 17.828 | 18.407 → 18.687 |
| `a1c80f2dcf82c75ab` | 18.040 → 18.332 | 18.669 → 18.960 |

There were 8 stamps and 8 deliveries. Every stamp is followed about 0.3 s later by a `tool_result` carrying the matching injection in the same subagent's transcript. No subagent went without a guide, and none got a duplicate. Adoption switched principals repeatedly inside the 2.7 s window: the stamps from the four subagents interleave.

**How this was counted, and two readings it corrects.** Injections were counted in each subagent's transcript from `tool_result` lines only. One transcript had two lines naming the marker. The second was that subagent's own hand-back quoting the marker (`type: assistant`, `SubagentHandback`), not a second delivery. Separately, one subagent's self-report said it received no bootstrap guide, while its transcript holds exactly one. So self-reports are not evidence for this check in either direction. The ledger-versus-transcript comparison is.

**What this does NOT prove.** It shows correct routing under real interleaved adoption. It cannot show that a call was in flight across a sibling's adoption, which is the window the original failure needed. That window is what `concurrent_principals_each_receive_their_own_first_call_guide` forces deterministically, and it stays the regression guard.

**Incidental findings from the same run, recorded where they belong:**
- Six path-scoped `symbols` zeros and two `references` `symbol not found` errors against a 0–3 s old rust-analyzer. Recorded in `docs/issues/archive/2026-07-18-symbols-overview-include-body-ignored-and-search-flake.md` and `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md`.
- On the **pre-fix** binary, the post-compact `workspace(post_compact=true)` call at about 14:45Z auto-injected `project-activation-bootstrap` into the main session. That stamp reached no ledger file on disk: the main ledger's entry is the new binary's re-delivery at 14:47:55Z, and no ledger file for this session was written between 14:43:00Z and 14:47:30Z (the one file written in that window, at 14:43:13Z, belongs to an unrelated CLI session, `7469d02d`, and records its own legitimate delivery). **Unexplained.** The process that did it has been replaced, so it is recorded here and not chased. If a single-principal session is ever seen receiving a guide its ledger does not record, reopen this with that evidence.

## References

- `docs/issues/archive/2026-09-24-residual-in-session-subagent-guide-starvation.md`: the starvation residual. That one is about parent vs subagent through a shared `session_id`; this one is about sibling vs sibling under correct per-principal stamps.
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`: the design this defect undermines.
- `a126bf48`, `971ed73f`: today's ledger fixes. Both are correct for sequential calls; this is the concurrent case.
- Workflow context: `deep-agent-workflow-observations:DWF-7`.

## Fix provenance

- **SHA:** `69e89228` (on `experiments`) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `3cafc22d19afe0209e0f48c352ddc3c007177e9c` — content hash of the diff; survives rebase and cherry-pick.

`fix(guide-ledger): each call keeps its own principal's ledger; adoption moves handles instead of swapping contents`
