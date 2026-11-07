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
`src/tools/symbol/references.rs`: `reask_while_unsettled` re-asks the LSP up to three times (delays 200, 600,
1200 ms) while the answer is unsettled, i.e. empty, or with no hit outside the definition file while
`corroborate_zero_references` finds the symbol in other files. It never accepts a smaller answer than the one
in hand and stops on an error. If the answer is still unsettled the existing `completeness_warning` fires as before.

## Tests added
In `src/tools/symbol/tests.rs`:
- `reask_while_unsettled_does_not_ask_again_when_the_first_answer_is_settled`
- `reask_while_unsettled_asks_again_until_the_answer_settles`
- `reask_while_unsettled_gives_up_when_the_delays_run_out`
- `reask_while_unsettled_keeps_the_answer_in_hand_when_a_reask_fails`
- `reask_while_unsettled_does_not_take_a_smaller_answer` (rewritten after the first version let a mutation survive)

They test the helper with fake refetch closures. The wiring in `References::call` is not covered by a test.

## Workarounds
Run `references` twice, or corroborate with `grep` / `call_graph(direction="callers")` as the warning says.

## Resume
Status stays `open` until the gate is green. Then: run the gate, confirm mutation 3 of the helper, and reproduce on
a cold pyright (restart the server, call `references` on a symbol used in other files) to see whether the first
call now returns the full set. A test of the `References::call` wiring is still missing.

## References
- `docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md`
