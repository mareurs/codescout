---
kind: bug
status: mitigated
title: 'BUG: first references call after start returns a partial result with a warning; an immediate retry returns the full one'
owners:
- marius
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: 2026-11-08
opened: 2026-11-07
severity: medium
unverified: 'Not reproduced, before or after the fix: one cold pyright start on Linux (1.1.412) returned the full set on the first call even without it. The re-ask is tested on its own loop; the wiring in References::call and the window check are not tested. The 5 s ceiling is a guess, not a measured warm-up time. A re-ask that answers [] can add one client wait of up to 5 s past the budget; not measured.'
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

The wait lives in the tool, not the client. `LspClient::references` re-asks only an EMPTY answer (`request_until_answered`). The reported answer was non-empty (2 hits, both in the definition file), so the client could not see the gap; only `References::call` can, by comparing it with the text scan.

`References::call` (`src/tools/symbol/references.rs`) now calls `reask_until_outside_hit` when all three hold: the client reports its cold-start window open (`LspClientOps::cold_start_window_open`, default `false`, `LspClient` overrides it), no location lies outside the definition file or a build directory, and `corroborate_zero_references` finds the symbol as a whole word in other files. It re-asks every 500 ms for at most 5 s (`PARTIAL_REASK_BUDGET`, `PARTIAL_REASK_INTERVAL`), keeps the answer in hand when a re-ask fails or answers `[]`, and on exhaustion returns the partial answer, which still gets the existing zero-external `completeness_warning`.

Why the two waits do not multiply (PR 33 measured 62 s): a non-empty answer costs the client no wait, so the tool's loop is the only wait in the partial case. A re-ask that answers `[]` can still cost one client wait of up to 5 s past the tool budget; that case was not measured.

## Fix provenance

- **SHA:** `9f78d028e27d8b0e38b34963218b357c9c9c13e5` (`experiments`)
- **patch-id:** `c6901954325299c0012bb0225f89e41da3c4b141`

## Tests added

`src/tools/symbol/tests.rs`, on `reask_until_outside_hit` and `has_outside_hit`: a partial answer is re-asked until an outside location appears; an answer that already has one is not re-asked; a server that never improves is asked for the budget and no longer; a failed re-ask and an empty re-ask keep the answer in hand; a build-directory location is not an outside hit. Two of them failed against a stub that returned its input; a mutant that drops the empty-answer guard fails `an_empty_re_ask_does_not_replace_the_partial_answer`. Gate green 2026-11-08: FMT, CLIPPY, LEAN 6239, DEFAULT 9131, Python 1764.

Not tested: the wiring in `References::call`. The mock reports the window closed, and a mock that never improves would hold a test for the full 5 s budget.

## Workarounds
Run `references` twice, or corroborate with `grep` / `call_graph(direction="callers")` as the warning says.

## Resume

Status `mitigated`: a retry exists for the reported shape, but the defect was never reproduced, so it is not shown to cure the report. To close: reproduce on a project like the reporter's (Windows, pyright 1.1.413, a slower first index) and confirm the first call returns the full set, or the warning stays and the re-ask ends at 5 s. Add a wiring test if the mock gains a window switch and a settable budget.

## References
- `docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md`
