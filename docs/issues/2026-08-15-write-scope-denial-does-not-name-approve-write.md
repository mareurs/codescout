---
id: '0a15c81150c4cce7'
kind: bug
status: open
title: 'BUG: a write-scope denial does not name `approve_write` — 26% of denials are followed by retrying the same denied write'
owners:
- marius
tags:
- write-guard
- approve_write
- agent-guidance
- usage-db-evidence
topic: prompt-surface-consistency
---

## Summary

When a write is refused for being outside the approved scope, the resolution is the `approve_write`
tool. Measured across the whole recorded corpus, only **43%** of denials are followed by calling it;
**26%** are followed by re-issuing the *same denied write*, which is refused again. The denial
appears not to make the escape reachable from where the agent is standing.

Small in absolute terms (42 occurrences) but a clean, high-repeat signature: it has the **highest
immediate-repeat rate of any error family in the corpus**, which is the specific shape of "the error
did not tell me what to do next".

## Symptom (Effect)

A write is refused as out-of-scope; the next call is the same write, refused identically. Observed
as `edit_file:error` (7) and `edit_markdown:error` (4) immediately following a `write_scope_denied`.

## Reproduction

Not reproduced synthetically — this is an aggregate finding from recorded sessions. To reproduce:
issue an edit to a path outside the approved write scope and observe whether the refusal names
`approve_write` and its required arguments.

## Environment

codescout on `experiments` @ `a7da09c6`. Evidence from 13 `.codescout/usage.db` files, 53,916 calls,
460 sessions, read 2026-08-15.

## Root cause

Unknown — **not yet read**. The error is classified as `write_scope_denied` by
`normalize_err_family` (`src/usage/db.rs`), but the message-producing site has not been inspected,
so whether the text omits `approve_write` entirely, mentions it without the arguments needed to call
it, or names it clearly and is simply missed, is undetermined. Stated as a hypothesis rather than a
conclusion: the behavioural evidence below shows the outcome, not the mechanism.

## Evidence

### Recovery split (42 denials)

    18  approve_write : success   (43% — correct resolution)
     7  run_command : success
     7  edit_file : ERROR         \  26% combined: the same denied
     4  edit_markdown : ERROR     /  write, re-issued and refused again
     3  workspace : success
     1  read_file : ERROR

### Highest immediate-repeat rate in the corpus

Per-family, the share of errors immediately followed by another error of the same family:

    write_scope_denied         26%
    replace_dropped_sibling    17%
    il5_edit_markdown_routing  17%
    il1_read_overlaps_symbol   14%
    il4_read_markdown_routing  13%
    symbol_not_found           13%
    json_path_unsupported       7%
    il3_shell_on_source         6%
    il2_structural_edit         6%
    il3_pipe_to_trimmer         3%

The bottom of this list is instructive: `il3_pipe_to_trimmer` repeats only 3% of the time because
its message states the corrective action concretely (run bare, then query the buffer). The families
at the top are the ones whose messages leave the next move ambiguous.

## Hypotheses tried

1. **Hypothesis:** 42 occurrences is too small to act on.
   **Test:** compare repeat *rate* rather than count against every other family.
   **Verdict:** rejected — rate is the meaningful measure for a guidance defect, and this family
   leads it. A denial that is re-attempted a quarter of the time is failing at its job regardless of
   volume.

## Fix

Not implemented, and the message site should be read before designing one. If the text does not
already name `approve_write` with the arguments needed to call it, adding that is the obvious move —
`il3_pipe_to_trimmer`'s 3% repeat rate is the demonstration that a concrete corrective action in the
message is what drives the repeat rate down.

## Tests added

None — filed on discovery.

## Workarounds

Call `approve_write` for the intended path, then re-issue the write.

## Resume

Find the site that emits the write-scope denial (start from `write_scope_denied` in
`normalize_err_family`, `src/usage/db.rs`, and work back to the emitter) and read the message text.
Confirm or refute the hypothesis that it omits `approve_write`. If confirmed, add the tool name and
its arguments to the hint, then re-measure the immediate-repeat rate — 26% falling toward
`il3_pipe_to_trimmer`'s 3% is the acceptance test.

## References

- `src/usage/db.rs` — `normalize_err_family`, where `write_scope_denied` is classified
- `docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md` — same probe, same
  class of defect (the corrective action is not reachable from the message)
- Evidence source: 13 `.codescout/usage.db` files, 53,916 calls, read 2026-08-15

