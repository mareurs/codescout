---
id: '27d66ed4f2d89fa4'
kind: bug
status: open
title: librarian accepts a declared key for an action that does not read it, and the response does not say so
tags:
- cluster/accepted-parameter-silently-dropped
opened: 2026-10-30
owner: marius
severity: low
---

# BUG: librarian accepts a declared key for an action that does not read it, and the response does not say so

## Summary
`librarian` now reports a key its schema does not declare (`ignored_params`, fixed in `fee5942bd` for bug `048fc80383fc1a75`). It does not report a key the schema DOES declare when the chosen action ignores it. `librarian(action="status", force=true, write=true)` returns a plain status result with no note, although `force` belongs to `reindex` and `write` to `legibility_scan` and `link_scan`.

## Class fit
`IC-15`, an accepted parameter that is silently dropped. The caller names a key that looks valid, because the schema lists it, and the action discards it with no sign in the response. It differs from `048fc80383fc1a75`, where the key was undeclared: here the schema itself invites the call, so the check cannot be a comparison against `input_schema()` properties.

## Symptom (Effect)
Observed 2026-10-30 on the rebuilt binary, which already served `ignored_params` for an undeclared key. The call `librarian(action="status", force=true, write=true)` returned `{"running": [], "note": "…"}` with no `ignored_params`. `force` is labelled `reindex:` and `write` is labelled `legibility_scan/link_scan:` in the schema. A caller who sends `force=true` to `link_scan` expecting a re-walk gets a normal report.

## Reproduction
1. Call `librarian(action="status", force=true, write=true)`.
2. Read the response. It has no `ignored_params`.

## Environment
codescout MCP server on branch `experiments`, rebuilt after `fee5942bd`, Linux.

## Root cause
The key-to-action labels live only in the `description` strings of `Librarian::input_schema` (for example `"reindex: ignore cached file hashes…"`). `Librarian::call` compares arguments to the schema's property names, so every declared key passes. No run-time table records which action reads which key. The only consumer of the labels is the test `Librarian::tests::every_action_labelled_schema_key_is_honored_by_that_action`, which checks the forward direction: that each labelled key is honoured by its action. Read in `src/librarian/tools/librarian.rs` on 2026-10-30.

## Evidence
The live call above. The probe test shows the label grammar is already machine-readable: `"context/reindex/workspace_state_at/link_scan: scope"` names several actions for one key.

## Hypotheses tried
1. **Hypothesis:** the `ignored_params` check already covers declared-but-unread keys. **Test:** `librarian(action="status", force=true, write=true)` after the rebuild. **Verdict:** rejected, no note in the response.

## Fix
Not started. Lead: derive the key-to-actions map from the same description labels the probe parses, and add the key to `ignored_params` when the chosen action is not among its labelled actions. A key labelled for no specific action must stay exempt. Risk: a label that is wrong or incomplete would then report a key the action does read, so the derivation needs the same probe that guards the labels today.

## Tests added
None. Status is `open`.

## Workarounds
Read the schema description of each key and send only the keys labelled for the action.

## Resume
Read `src/tools/param_probe.rs` (`sweep`) for how it splits a label into actions, then decide whether to share that parser with `Librarian::call`.

## References
`docs/issues/2026-10-30-librarian-reindex-link-scan-and-audit-doc-refs-accept-a-path-argument-and-run-project-wide-without-saying-so.md`; `src/librarian/tools/librarian.rs`; `src/tools/param_probe.rs`.
