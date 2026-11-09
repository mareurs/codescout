---
id: '84029483d2895d9b'
kind: bug
status: open
title: A path-pinned generic call_graph callee lookup returned unrelated short-name call chains
tags:
- call-graph
- lsp
- cluster/unclassified
opened: 2026-11-09
owner: marius
severity: medium
unverified: Observed on the older running shared MCP binary9e4db9b9; current-source isolated reproduction and cause not yet established.
---

# BUG: a path-pinned generic call_graph callee lookup returned unrelated chains

## Summary

During ECC create-finalization dogfood, a call_graph request pinned to src/librarian/tools/create.rs::call, direction=callees, returned run_command call chains and a router call site rather than the create body inspected with symbols. The response was not used as evidence about create's outgoing calls.

## Class fit

The observed relation population did not match the requested file/symbol boundary. A duplicate-short-name resolution issue, stale LSP coordinates and running-binary/source drift remain distinct unverified explanations, so unclassified is retained pending reproduction instead of forcing a mechanism class.

## Symptom (Effect)

Actual response: artifact.rs:548 call→call; run_command/mod.rs:468,512 call→run_command_inner; run_command/inner.rs:870 run_command_inner→handle_successful_output_with. The inspected create::call body uses catalog and filesystem operations and does not establish those outgoing run_command chains. A consumer could analyze another tool's flow under the requested name.

## Reproduction

call_graph(symbol=call,path=src/librarian/tools/create.rs,direction=callees,max_depth=2,detail_level=full,workspace=/home/marius/work/claude/codescout). Exact captured response and caveats are retained in [incidental navigation evidence](../trackers/ecc-integration/evidence/2026-11-09-create-finalization-dogfood/incidental-navigation.txt).

## Environment

Shared running MCP pid1520813, git_sha a52d68284, dirty=true, build_id9e4db9b94323fb5bc65e3af403904f883459979206077488e448a362291620dc, executable deleted. Main source038fffb3 at observation. This is not a current-source binary reproduction. Earlier private fixture routing audit measured a unique Python symbol and does not cover duplicated Rust call names.

## Root cause

Unknown. Reproduce with fresh current-source private server and two same-name Rust functions before identifying the responsible resolver or direction logic.

## Evidence

Exact one observed response plus inspected source body; current-source regression not run. Active bug-title query graph returned only an unrelated guide-topic issue; focused historical text search found no matching report. These searches do not prove global novelty.

## Hypotheses tried

None attributed as cause. Source symbols and references were used to continue the finalization task safely.

## Fix

Pending separate navigation cycle. No source change in current finalization correction.

## Tests added

None yet: incidental older-binary observation requires a fresh isolated reproducer before choosing a regression.

## Workarounds

Do not trust a graph that contradicts the named function body. Inspect exact source and references, then reproduce with fresh current runtime.

## Resume

Open, unclaimed follow-up after create finalization. Keep the uncertainty about binary/source identity explicit.
