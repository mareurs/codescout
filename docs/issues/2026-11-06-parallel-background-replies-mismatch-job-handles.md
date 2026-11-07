---
id: '0054f6967e09530f'
kind: bug
status: open
title: Parallel background command replies can name each other's job handles
tags:
- cluster/unclassified
- tooling
opened: 2026-11-06
unverified: Not reproduced. The server's own binding of handle to command is pinned by a test and seen right on one live pair from Claude Code. The transport and the Codex client, where it was observed, were not tested.
---

# BUG: Parallel run_command background replies disagree with the command bound to their handles

## Class fit

A background identity attribution mismatch: a reply's output_id/record should name the command from that same invocation. This differs from a process completing unsuccessfully; both commands ran, but later handle reads identify the opposite command. Observed during read-only cleanup inventory, before any deletion.

## Symptom (Effect)

Two independent run_command calls were started concurrently through Promise.allSettled: docker system df returned @bg_000000cf and the du command returned @bg_000000d0. Later tail responses bind cf to `du -x -h --max-depth=1 /home/marius/.cache/codescout-resident /home/marius/work/claude/setup-ab-data /tmp`, and d0 to `docker system df`. Both command labels and complete outputs were inspected; no count or cleanup decision was inferred from the wrong assumed command. Root server455879. The concurrently active campaign @bgcc was started sequentially and is separately pinned by its record/PID.

## Evidence

Original paired tool replies are in this thread's rollout. Later persisted records `/tmp/codescout-bg/455879-a720241d-bg_000000cf.json` and corresponding d0 record retain the actual command bindings; tail outputs label their own commands. This is one observed pair, not a measured frequency or proven source root cause.

## Workaround

Start background operations sequentially. Before reading or acting on a background job, verify the record's command, owner server and PID rather than trusting invocation adjacency. No tool production changes in this cleanup.

## Resume

**Not reproduced in the server, 2026-11-07.** Read by code: `spawn_background_command` (`src/tools/run_command/inner.rs`) takes one `ref_id` from `OutputBuffer::store_background` and uses it for the stored job, for the record file path and for the reply. The three come from one call, so the server has no step where one call can return the handle of another.

Two checks, both without a swap:
- Test `concurrent_background_calls_each_get_the_handle_of_their_own_command` (`src/tools/run_command/tests.rs`, commit `2abaca44a`). It starts 20 concurrent pairs through `run_command_inner`. For each reply it checks the job behind the handle, the record file and the hint. It has no red phase and no mutant: there was no defect to make it fail on.
- Live, one pair, Claude Code client: `echo live-marker-A` returned `@bg_00000005` and `echo live-marker-B` returned `@bg_00000006`. `tail` on each handle printed its own marker and named its own command.

**What this does not cover.** The report came from a Codex thread that used `Promise.allSettled`. The test bypasses the MCP transport. One live pair is not a frequency. The open suspect is the pairing of replies to calls in that client or in the transport, not the server's handle allocation. To test it, repeat the pair from a Codex session, or send many concurrent pairs through the stdio transport.

Until then keep the workaround: verify a job's command in its record before acting on its output.
