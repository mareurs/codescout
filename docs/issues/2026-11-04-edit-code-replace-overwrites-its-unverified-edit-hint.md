---
id: '053ca236198f1b9b'
kind: bug
status: fixed
title: edit_code replace overwrites its own "edit NOT verified against dropped symbols" hint with the verify-callers hint
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-05
opened: 2026-11-04
related:
- '95abaa31c77daa39'
severity: medium
unverified: No test forces the Unverified verdict through the replace arm; attach_advice_keeps_an_existing_safety_warning covers only the helper the arm now uses.
---

# BUG: edit_code replace overwrites its own "edit NOT verified against dropped symbols" hint with the verify-callers hint

## Summary

When `edit_code` `replace` cannot run its dropped-symbol check, `do_replace` sets `hint` to "The edit was written but NOT verified against dropped symbols. Re-read the file with symbols(path) to confirm it is intact." The caller, `call()`, then assigns `result["hint"]` again with the verify-callers text, so the safety warning is lost. The structured fields `corruption_check` and `corruption_check_error` remain.

## Class fit

No class in the roster fits: the defect is two writers of one single-valued field, where the later write silently replaces the earlier one. It is nearest to a lost-message problem rather than to the recorded classes, so `cluster/unclassified` is used instead of forcing a fit. It differs from the usual `unclassified` members only in that both writers are in one file.

## Symptom (Effect)

A `replace` whose post-edit symbol extraction failed returns `corruption_check: "skipped"` plus the verify-callers hint, but not the sentence telling the agent the edit is unverified and to re-read the file. An agent that reads `hint` and not `corruption_check` does not learn that nothing was verified. Under an endorsed-hint regime (see `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md`) the surviving text would be the only one with authority.

## Reproduction

Read `src/tools/symbol/edit_code.rs` on `experiments` at `4d659fe57`: `do_replace` sets `response["hint"]` at line 1512 inside `if verdict == CorruptionVerdict::Unverified`, and returns `Ok(response)`; `call()` then runs `result["hint"] = json!(verify_callers_hint(name_path, rel_path));` at line 330 with no check for an existing value. Not reproduced at run time: forcing the `Unverified` verdict was not attempted.

## Environment

codescout on `experiments` at `4d659fe57`; 2026-11-04.

## Root cause

`hint` is one string per result. Two code paths write it and the later write does not look at the earlier one. Read from the source; no history was traced.

## Evidence

- Source lines above, read 2026-11-04. The `remove` action has the same warning shape at line 1159 and is not followed by the callers hint (its `call()` arm returns `do_remove` directly), so only `replace` loses it.
- Raised by the builder check of the harness-delivered hints design (a subagent's report); the lines were then read by hand.

## Hypotheses tried

N/A — the source settles the order of writes.

## Fix

Applied. `do_replace` no longer loses its safety warning: the `replace` arm of `call()` in `src/tools/symbol/edit_code.rs` no longer assigns `result["hint"]`. The callers text now travels in a separate `advice` field `{text, verified}` through `attach_advice`, which leaves an existing `hint` (the "NOT verified against dropped symbols" warning) untouched. `advice` appears only when the declaration changed and `check_callers` finds or cannot rule out an outside caller. `format_compact` renders the hint first and the advice after it.

- Fix commit `08775f958` on `experiments` (merged by fast-forward at `35c1264f1`). It also carries the fix for bug `66b133ffaa36de89`. Follow-up `2f1395f56` moves the callers check after the shrink guard.
- Gate green on `35c1264f1`: FMT 0, CLIPPY 0, LEAN 6063/6063, DEFAULT 8931/8931.

## Fix provenance

- **SHA:** `08775f9583cf48ad86f453daccea4081f74952ee` (`experiments`)
- **patch-id:** `a25414ce11e83aa5f05e84d4e4c0ab117a92383c`

## Tests added

- `attach_advice_keeps_an_existing_safety_warning`, `src/tools/symbol/tests.rs:11742`: an existing `hint` survives `attach_advice`, and `advice` is a separate field. This covers the helper the arm now uses.
- `edit_code_replace_of_a_body_only_edit_carries_no_advice_and_no_hint`, `src/tools/symbol/tests.rs:11685`, and the other `edit_code_replace_*` advice tests added in the same commit cover the arm's `advice` shapes.
- **Not covered:** no test forces the `Unverified` verdict through the whole `replace` arm. The test that this bug's `## Fix` section asked for does not exist.

## Workarounds

Read `corruption_check` in the result; it is set when the check was skipped.

## Resume

1. Write the arm-level test that forces an `Unverified` verdict on `replace` and asserts the warning text survives next to `advice`. Then discharge the `unverified:` caveat and archive this file.

## References

- `src/tools/symbol/edit_code.rs` (`do_replace`, `call`, `verify_callers_hint`)
- `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md`
