---
id: f0d4a3093cbb9b63
kind: bug
status: open
title: 'BUG: rekey_prefix does not move a ledger''s own cite when a lowercase or numeric run precedes the colon'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-13
severity: low
unverified: 'Read from the regex in retarget_prose, not shown by a test. Not established: that any real ledger prose has such a shape.'
---

# BUG: rekey_prefix does not move a ledger's own cite when a lowercase or numeric run precedes the colon

## Summary
`retarget_prose` (`src/librarian/catalog/rekey.rs:222`) reads `<name>:F-1` as a qualified cite. If `<name>` is not the ledger's own slug or key, the token stays. Prose such as `at 12:F-1` or `v2:F-1` matches the qualifier group (`[a-z0-9][a-z0-9_-]*`) and is therefore left unmoved when the prefix `F` is rekeyed. The same input with an uppercase run before the colon (`T9:F-1`) is also left alone, by the continues-a-name guard.

## Class fit
cluster/unclassified: the closed grammar has no escape for a number or version tag written before a colon, so the rewriter must guess. It guesses "foreign cite" and keeps the token.

## Symptom (Effect)
After `rekey_prefix F→X`, a mention such as `see v2:F-1` still says `F-1` while its entry is now `X-1`.

## Reproduction
Not run. Read path: the regex `(?:(?P<q>[a-z0-9][a-z0-9_-]*):)?\bF-(?P<n>\d+|N)\b` captures `q="v2"`; the arm for a qualifier that is neither `own_slug` nor `own_key` returns the match unchanged.

## Environment
experiments at 7db510676 (final-wave fix b146e18a3 introduced the digit-leading qualifier).

## Root cause
A qualifier is recognised by shape alone. The rewriter cannot tell a name from a number or version tag.

## Fix
Parked by the Plan C1 final review (finding M-2). Options: accept the gap and document it in the `rekey_prefix` description, or require a real key/slug to resolve before treating the run as a qualifier. Also correct the doc example at `rekey.rs:217` (M-1): `[[2026-11-ledger:F-1]]` yields the whole name `2026-11-ledger`, not `ledger`, and is left alone as foreign.

## Resume
Open. Not started.

## References
- `src/librarian/catalog/rekey.rs:222`
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
