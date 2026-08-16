---
id: '26a793b432e0f29c'
kind: bug
status: fixed
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

Read 2026-08-16. **The title of this bug is wrong**, and the mechanism is worse than it claims.

The filing offered three possibilities: the text omits `approve_write` entirely, mentions it
without the arguments needed to call it, or names it clearly and is simply missed. The answer is
the second. `validate_write_path` (`src/util/path_security.rs`) emitted:

```
write denied: '<path>' is outside the project root.
Call approve_write('<dir>') first to grant write access for this session.
```

Two defects in one clause:

1. **`'<dir>'` is a literal placeholder.** The reader has to work out which directory to approve.
2. **`approve_write('<dir>')` is not a callable shape at all.** The tool takes a **named** `path`
   parameter (`src/tools/approve_write.rs`, `required: ["path"]`). An agent following the message
   verbatim earns a *second* error.

That reframes the behavioural evidence below. The 26% immediate-repeat rate is not "the agent never
saw the remedy" — it is "the agent saw a remedy it could not execute". The `il3_pipe_to_trimmer`
comparison still holds and is now sharper: that message repeats at 3% because its corrective action
can be run as written.

**The directory was in hand the whole time.** `WritePathDecision::OutsideRoot { resolved: PathBuf }`
carries it, and `src/tools/core/write_ack.rs` already derives the approvable directory from
`resolved.parent()` when minting an ack handle. The `bail!` arm matched `OutsideRoot { .. }` and
discarded it.

**Second finding.** The `write_scope_denied` family also covers two *hard* denials — an unresolved
`..`, and a deny-listed location — which named no remedy at all. Correct as far as it went, since
neither is approvable, but a reader who has learned "write denied → approve_write" from the
approvable case spends a call finding that out.
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

Fixed on `experiments` in `fe7732e2`.

**The approvable case now names a call that can be run verbatim.** The arm binds `resolved` instead
of discarding it and renders the real directory in the named-parameter form:

```
write denied: '/var/x/y.rs' is outside the project root.
Call approve_write(path="/var/x") first to grant write access for this session.
```

Same derivation `write_ack.rs` already used for the ack handle, so the two paths agree on which
directory is at stake.

**The two hard denials now say approving will not help, and why.** The `..` case explains the path
itself is rejected rather than its location; the protected-location case explains the deny-list is
checked first and holds even inside an approved directory. Saying what will *not* work is the point
— both share a family with the approvable case, so silence there reads as "try approve_write".

The `"write denied"` prefix is unchanged on every path: `usage::db::normalize_err_family` keys on it
to classify this family, and that is the measurement this fix should be judged by.
## Tests added

Two, in `src/util/path_security.rs`.

- `write_denial_names_an_approve_write_call_that_can_be_run_verbatim` — asserts the message is
  **executable**, not merely that it mentions the tool: the real directory appears, `<dir>` does
  not, and the call is in the `approve_write(path=` form. Also pins the `write denied` prefix so
  the error-family classification keeps working. Mutation-verified: restoring the placeholder
  reproduces the filed message verbatim and fails the test.
- `hard_denials_say_that_approve_write_will_not_help` — pins the negative guidance on the
  unapprovable path.

One correction worth recording, because it is the same class of error as the bug. My first version
of the second test used `/var/../../etc/x.rs`, expecting the `..` branch. It canonicalizes cleanly
and takes the `OutsideRoot` arm instead — that branch fires **only when an intermediate directory
does not exist**, which the branch's own comment states. The test now uses a non-existent
intermediate. A test whose premise is wrong asserts nothing, and this one failed loudly only
because it was written to `panic!` rather than skip.

Gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` clean, `cargo test --lib` 3756 passed
/ 0 failed / 7 ignored.
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
