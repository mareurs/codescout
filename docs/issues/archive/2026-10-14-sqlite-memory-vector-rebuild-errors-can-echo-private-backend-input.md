---
id: 983a6a2215c4b9c5
kind: bug
status: fixed
title: SQLite memory rebuild exposes private backend error text in its diagnostic chain
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- albert-follow-up
closed: 2026-10-14
opened: 2026-10-14
owner: marius
severity: high
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
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

Fixed and archived after source `c773a2ca5b5225010c06b5784a13470319bc4617` landed on experiments and the final ordered gate passed. See [the implementation evidence and rulings](../../superpowers/plans/2026-10-14-albert-recovery-hardening.md). Operator native/corpus acceptance is tracked separately.

## References

Albert's sanitized pachet-fix-codescout-2026-10-14.zip; its candidate remains separate from the supported CLI.

## Fix provenance

- **SHA:** `c773a2ca5b5225010c06b5784a13470319bc4617` (`experiments`)
- **patch-id:** `1e6a43618eb28337de242fcfb31271132a9b0354`

Verified final ordered gate: FMT0 CLIPPY0 LEAN0 PYTHON_LIGHT0 DEFAULT0, with normal temporary paths and eight test threads. SQLite27pass, ordinary CLI4pass and genuine two-record MiniLM CLI1pass; credential-free Python light1223pass/424subtests. Both review coverage gaps have observed mutation RED and restored GREEN.
