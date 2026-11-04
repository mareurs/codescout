---
id: f5e732b0efeebd9e
kind: bug
status: open
title: edit_code rename's verify-callers hint names the symbol as it was before the rename
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
opened: 2026-11-04
related:
- '95abaa31c77daa39'
severity: low
unverified: Read from source only; a rename was not run; no fix, no test.
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

Not applied. Build the hint from the new name (the parent path of `name_path` plus `new_name`). Whether `rename` should carry the hint at all is a design question, since the LSP rename already updates the call sites; the design above keeps it, unverified, in `hint`.

## Tests added

N/A — no fix applied. A fix needs a test that a rename's hint reads back to the new name and the path (the existing test `the_verify_callers_hint_reads_back_to_the_symbol_and_the_path` covers only the helper, not which name each arm passes).

## Workarounds

Call `references` on the new name by hand.

## Resume

1. Decide whether `rename` keeps the hint.
2. If it does, pass the new name path and add the arm-level test.

## References

- `src/tools/symbol/edit_code.rs` (`call`, `verify_callers_hint`)
- `src/tools/symbol/route_path_tests.rs` (`the_verify_callers_hint_reads_back_to_the_symbol_and_the_path`)
- `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md`
