---
id: ef3461bba39449f6
kind: bug
status: taken
title: 'BUG: mjs modules are excluded from code indexing and size preflight'
tags:
- cluster/selector-narrower-than-its-population
- indexing
- javascript
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
closed: null
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
`.codescout/resident-private/albert-audit-20261016/mjs-red.log`; Albert maintainer audit P-02. His patch was not found in Downloads; this is an independent implementation of the reported mapping correction, not a byte comparison with his patch.

## Hypotheses tried
1. Shared extension allowlist blocks existing parser support — confirmed by both consumer regressions failing before the repair.

## Fix
Add mjs to the JavaScript mapping, preserving existing parser, ignore, schema and protocol behavior. Fix commit identifiers pending verification.

## Tests added
`mjs_stream_index_keeps_modules_and_respects_ignores` exercises actual parsing/index walk with configured and Git ignores. `mjs_preflight_counts_modules_toward_size_limit` checks eligible byte totals and threshold behavior.

## Workarounds
Use a build with the mapping correction. Renaming source files is unnecessary once corrected.

## Resume

Both regressions now pass (2 passed, 0 failed). Independent review and full ordered gate are running; commit verified source and record SHA plus patch-id after integration.

## References
Albert maintainer audit P-02; src/embed/mod.rs; src/embed/preflight.rs; src/retrieval/sync.rs.
