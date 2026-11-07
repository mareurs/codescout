---
kind: bug
status: open
title: 'BUG: first references call after start returns a partial result with a warning; an immediate retry returns the full one'
owners:
- marius
tags:
- cluster/lazy-warmup-bills-the-first-caller
opened: 2026-11-07
severity: medium
unverified: Still one observation, never reproduced on a cold pyright. The re-ask is covered by unit tests of the retry decision only, not by a real cold start. Mutations 1 and 2 of the helper were killed; mutation 3 was not confirmed. The full gate has not been run.
---

# BUG: first references call after start returns a partial result with a warning; an immediate retry returns the full one

## Summary
On a cold pyright, the first `references` call returns only the definition file's hits and a
`completeness_warning`; the same call repeated immediately returns the complete set. The tool already
detects the gap but hands the retry to the caller.

## Class fit
Work deferred to first use is billed to the first caller, who gets a well-formed answer that is a
subset of the true one. It differs from the class's other members in that the guard fires and names
the problem, so the answer is not a silent zero; the open point is that the tool detects the gap and
still returns the partial set instead of waiting.

## Symptom (Effect)
Reported once by a peer session on a Python project, home project activated moments earlier,
`references(symbol="build_chain", path=...)`:
1. first call: 2 hits, both in the definition file, with the warning "LSP returned 0 references outside the
   definition file, but `build_chain` appears as a whole word in 3+ other source file(s) ... the reference
   index may still be warming after a reindex. Re-run, ...";
2. same call, repeated: "17 references in 3 files".

## Reproduction
Not reproduced by the filer: a `references` call on the same symbol returned the 17 hits on the first call
(index already warm). Needs a cold pyright start.

## Environment
Windows, pyright 1.1.413, Python 3.11, `pyright-langserver.cmd` from the npm shim.

## Root cause
Not established. The warning is raised in `References::call` (`src/tools/symbol/references.rs`) by the
external-refs-zero branch, after `corroborate_zero_references` finds the symbol as a whole word in other files.

## Evidence
The two outputs above. Related: `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md`.

## Hypotheses tried
None.

## Fix

Not applied on this branch. PR 33 proposed `reask_while_unsettled` in `src/tools/symbol/references.rs` and a longer
`not_answered_budget` (5 s to 15 s) in `src/lsp/client.rs`. Review measurements on Linux, 2026-11-07, with a fake
LSP server that answers every request of one method with a fixed answer inside the cold-start window (one run each):

- One `client.references` call, server answers `[]`: 5.04 s and 26 requests on the base, 15.14 s and 76 requests with
  the 15 s budget.
- `document_symbols`, server answers `null`: 5.06 s on the base, 15.16 s with the 15 s budget.
- One `references` call with the proposed re-ask loop around it, server answers `[]`: 62.43 s and 304 requests.

The 62 s is a worst case: a server that never leaves `[]`. `LspClient::references` already re-asks an empty answer
for `not_answered_budget`, so a second retry layer in the tool multiplies the wait. The proposed predicate also reads
the raw locations, before the build-directory and scope filters that `external_refs` uses. A fix must decide where
the wait belongs (the client or the tool, not both).

## Tests added

None on this branch. The helper tests from PR 33 were not brought over with the helper.

## Workarounds
Run `references` twice, or corroborate with `grep` / `call_graph(direction="callers")` as the warning says.

## Resume

Status stays `open`. Decide where the wait belongs, then reproduce on a cold pyright (restart the server, call
`references` on a symbol used in other files) to see whether the first call returns the full set. A test of the
`References::call` wiring is still missing.

## References
- `docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md`
