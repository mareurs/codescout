---
id: 66b133ffaa36de89
kind: bug
status: fixed
title: edit_code rename's verify-callers hint names the symbol as it was before the rename
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
closed: 2026-11-05
opened: 2026-11-04
related:
- '95abaa31c77daa39'
severity: low
unverified: 'CLEARED 2026-11-05. Was: read from source only; no fix, no test.'
---

# BUG: edit_code rename's verify-callers hint names the symbol as it was before the rename

## Summary

After a successful `edit_code` `rename`, the result carries `hint: verify callers: references(symbol="<old name>", path=...)`. The symbol now has the new name, so the call the hint tells the agent to copy cannot find it.

## Class fit

This is `doc-contradicted-by-code`: the hint is a statement of what to run next, and the state of the code after the rename contradicts it, because the name it points at no longer exists. It differs from the class's usual members because the text is generated at run time from the call's own arguments, not written once in a doc.

## Symptom (Effect)

An agent that copies the hinted call gets a not-found for the old name instead of the callers of the renamed symbol. Under an endorsed-hint regime (see `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md`) the agent would be told by an authority to run it.

## Reproduction

Read `src/tools/symbol/edit_code.rs` on `experiments` at `4d659fe57`, the `"rename"` arm of `call()`: `new_name` is read from the input, `do_rename(ctx, name_path, rel_path, new_name)` runs, then `result["hint"] = json!(verify_callers_hint(name_path, rel_path));` at line 275 uses `name_path`, the old name. Not run at run time.

## Environment

codescout on `experiments` at `4d659fe57`; 2026-11-04.

## Root cause

The `replace` arm and the `rename` arm share one helper call; for `rename` the second argument should be the new name path. Read from source; no history traced.

## Evidence

- The lines above, read 2026-11-04.
- Raised by the builder check of the harness-delivered hints design (a subagent's report); the source was then read by hand.
- Related: `verify_callers_hint` is set on every successful `replace` and `rename` without checking that the symbol has callers; the LSP rename has already rewritten the call sites.

## Hypotheses tried

N/A.

## Fix

Applied. The `rename` arm of `call()` in `src/tools/symbol/edit_code.rs` now builds the hint from the new name: `verify_callers_hint(&renamed_name_path(name_path, new_name), rel_path)`. `renamed_name_path` replaces the last segment of the name path with `new_name`. The hint is kept on `rename` (the design keeps it, unverified).

- Fix commit `08775f958` on `experiments` (merged by fast-forward at `35c1264f1`). It also carries the fix for bug `053ca236198f1b9b`.
- patch-id `a25414ce11e83aa5f05e84d4e4c0ab117a92383c` (`git show 08775f958 | git patch-id --stable`).
- Gate green on `35c1264f1`: FMT 0, CLIPPY 0, LEAN 6063/6063, DEFAULT 8931/8931.

## Fix provenance

- **SHA:** `08775f9583cf48ad86f453daccea4081f74952ee` (`experiments`)
- **patch-id:** `a25414ce11e83aa5f05e84d4e4c0ab117a92383c`

## Tests added

- `renamed_name_path_replaces_the_last_segment` and `rename_hint_reads_back_to_the_new_symbol_and_the_path`, `src/tools/symbol/tests.rs:11759` and `:11767`. They cover the helper pair the arm calls.
- Arm level: `src/tools/symbol/route_path_tests.rs` (`unix_only`) runs a rename and asserts the hint reads back to the new symbol `g` and the path (`assert_hint_names(hint, symbol, rel, what)`).

## Workarounds

Call `references` on the new name by hand.

## Resume

Closed. Nothing owed.

## References

- `src/tools/symbol/edit_code.rs` (`call`, `verify_callers_hint`)
- `src/tools/symbol/route_path_tests.rs` (`the_verify_callers_hint_reads_back_to_the_symbol_and_the_path`)
- `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md`
