---
id: '38a970a0ff4e71e7'
kind: bug
status: taken
title: SQLite memory rebuild exposes private backend error text in its diagnostic chain
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
opened: 2026-10-14
owner: marius
severity: high
---

# BUG: SQLite memory rebuild exposes private backend error text in its diagnostic chain

## Summary

Embedding errors are propagated with anyhow context, so an embedding backend that echoes a document includes that private text in the operator's error chain.

## Symptom (Effect)

See the observed reproduction; daily databases were not used.

## Reproduction

Base b27b0ab3, isolated albert-r02-hardening-20261014 worktree. Run the named regression against the unchanged implementation.

## Environment

Linux, Rust real SQLite/vec0 and Python3.14; controlled synthetic files and clean subprocess homes.

## Root cause

Embedding errors are propagated with anyhow context, so an embedding backend that echoes a document includes that private text in the operator's error chain.

## Evidence

Initial @bg_0000003e used the synthetic SYNTHETIC_PRIVATE_MARKER and confirmed it remained visible in formatted error diagnostics. No actual private memory was used.

## Hypotheses tried

The failure was reproduced before the source fix. No live migration or subscription call was used.

## Fix

Replace the backend error with a row-ID-only failure; keep original-index-retained context without exposing backend text.

## Tests added

embedding_failure_diagnostics_do_not_echo_private_backend_text.

## Workarounds

Use a controlled coherent trial and the follow-up recovery guide; keep all writers stopped.

## Resume

Implementation and full gate in progress. See ../superpowers/plans/2026-10-14-albert-recovery-hardening.md.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.
