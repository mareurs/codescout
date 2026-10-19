---
id: e8ba5df2b174fc4c
kind: bug
status: open
title: 'BUG: a pinned project''s memory vectors are written to the DEFAULT project''s `.codescout/embeddings/<name>.memories.db`'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-19
owner: marius
severity: low
unverified: 'Reported by an independent review of bdf34bdb4..46329d271 (evidence: scratchpad/rv-pin2/probe1.json, probe3-base.json); NOT reproduced by the session that filed this file. The root cause is a hypothesis.'
---

# BUG: a pinned project's memory vectors are written to the DEFAULT project's embeddings database

## Summary

Reported by the review of `bdf34bdb4..46329d271`: a `memory` write pinned at project B stores its vectors in the session's DEFAULT project A, under `A/.codescout/embeddings/<name>.memories.db`, not under B. The reviewer's pointer is `src/agent/mod.rs` around lines 2416-2433 (the semantic memory store bootstrap).

## Class fit

A per-call target (the pin) is honoured for the file the tool writes but not for the side store that is derived from it, which still resolves through the session default. It is the pin-is-per-call property failing at a second consumer.

## Symptom (Effect)

After a pinned memory write, project B's `.codescout/embeddings/` has no memories database and project A's does, named after B's memory.

## Reproduction

Not reproduced by the session that filed this. The reviewer's observations are in `scratchpad/rv-pin2/probe1.json` and `probe3-base.json` (of the session that ran the review).

## Environment

Linux, debug build of branch `fix/workspace-pin-2026-10-18` after commit `46329d271`; the same was seen on the base commit `bdf34bdb4` (`probe3-base.json`).

## Root cause

Unknown. Hypothesis, not tested: the semantic memory store is a single lazily-built `OnceCell` on `Agent`, keyed to the default project when first built.

## Evidence

The reviewer's probe files only.

## Hypotheses tried

None by the filing session.

## Fix

Not implemented. Candidate: key the semantic memory store by the resolved project root, as the pin resolution is.

## Tests added

N/A: not fixed; this file records a finding.

## Workarounds

Activate the project instead of pinning it when its memory search results matter.

## Resume

Read `Agent::semantic_memory` and its bootstrap near `src/agent/mod.rs:2416`; write a test that pins a second project, writes a memory, and lists both projects' `.codescout/embeddings/`.

## References

- `src/agent/mod.rs`
