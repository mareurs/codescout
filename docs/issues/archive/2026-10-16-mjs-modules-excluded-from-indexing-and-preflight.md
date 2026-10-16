---
id: 7e675057b406d83d
kind: bug
status: fixed
title: 'BUG: mjs modules are excluded from code indexing and size preflight'
tags:
- cluster/selector-narrower-than-its-population
- indexing
- javascript
closed: 2026-10-16
opened: 2026-10-16
owner: marius
related: []
severity: medium
---

# BUG: mjs modules are excluded from code indexing and size preflight

## Summary
The JavaScript parser supports modules, but the shared extension mapping rejects `.mjs` files before they reach it. Both code indexing and the size-confirmation preflight miss them.

## Class fit
The shared extension selector excludes a legitimate JavaScript module population while returning a well-formed empty index or Clear preflight verdict. Unlike a display-only count omission, these files never reach the indexer or the safety estimate at all.

## Symptom (Effect)
A temporary project containing two eligible `.mjs` modules indexed zero chunks. A project whose eligible module exceeded its size threshold returned Clear.

## Reproduction
Measured 2026-10-16: `./scripts/with-slot.sh cargo test --lib mjs_` before the mapping repair: 0 passed, 2 failed. Tests use temporary sources and a deterministic embedder; no native laptop/provider acceptance is claimed.

## Environment
Linux; experiments base 0007d926 plus a6ec001c in an isolated worktree.

## Root cause
`src/embed/mod.rs` lang_for_ext maps js/jsx to JavaScript but omits mjs. `stream_index` and `check_index_scope` both use that mapping.

## Evidence

Local RED/GREEN and frozen gate logs: `.codescout/resident-private/albert-audit-20261016/`. The two consumer regressions failed before the mapping and passed after it. Independent review's omitted-mjs mutation killed both tests. The final frozen full gate exited0: FMT0, CLIPPY0, LEAN0, DEFAULT0, PYTHON_LIGHT0. Lean library4513passed/8ignored; default library6865passed/9ignored; Python1223passed+424subtests. Counts overlap and are not a unique-test total.

Albert maintainer audit P-02 supplied the finding. His original patch was not available in Downloads; this is an independent implementation, not a byte comparison. Heavy Python, server-stack, real ONNX-weight suites and native laptop activation are outside this gate. The precommit dead-artifact-ID helper skipped because no runnable binary was available in its configured lookup paths; catalog records are checked through MCP separately.

## Hypotheses tried
1. Shared extension allowlist blocks existing parser support — confirmed by both consumer regressions failing before the repair.

## Fix

Add mjs to the JavaScript mapping, preserving parser, ignore, schema and protocol behavior. Exact commit identifiers are recorded in Fix provenance below.

## Fix provenance

- **SHA:** `28dc9e855f45c305dd4bb314c2478fb6ae30c9e5` (`experiments`)
- **patch-id:** `f770373a7aaf284d8eaef569dca0cac0097a402e`

## Tests added
`mjs_stream_index_keeps_modules_and_respects_ignores` exercises actual parsing/index walk with configured and Git ignores. `mjs_preflight_counts_modules_toward_size_limit` checks eligible byte totals and threshold behavior.

## Workarounds
Use a build with the mapping correction. Renaming source files is unnecessary once corrected.

## Resume

N/A — fixed and verified on experiments. Runtime activation is a separate operator action.

## References
Albert maintainer audit P-02; src/embed/mod.rs; src/embed/preflight.rs; src/retrieval/sync.rs.
