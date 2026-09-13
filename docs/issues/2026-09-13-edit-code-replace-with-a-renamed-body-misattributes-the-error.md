---
id: '493feb07511a15b0'
kind: bug
status: open
title: 'BUG: edit_code(action="replace")''s error for a body whose fn name differs from `symbol` misattributes the cause, sending the caller to change the body rather than the action'
owners:
- marius
tags:
- cluster/unclassified
- codescout-tool
- edit_code
- error-messages
closed: null
opened: 2026-09-13
owner: marius
related: []
severity: low
---

## Summary

`edit_code(action="replace", symbol="OLD_NAME", body="fn NEW_NAME() { ... }", attributes=[...])`
— i.e. supplying a `body` whose function name differs from `symbol`, in an attempt to rename and
re-body a function in one call — is refused with:

> edit_code replace('OLD_NAME') dropped the symbol definition — body must be the complete
> declaration (attributes, doc comments, signature, and body), not just body statements. File
> restored.

The message's stated diagnosis (*"not just body statements"*) is not what happened: the supplied
`body` **was** a complete declaration — full signature, braces, everything — it just declared a
different name than `symbol`. The message describes the wrong defect and sends the reader to fix
the wrong thing.

## Symptom (Effect)

A caller reads *"you passed body statements, not a full declaration"*, goes back and re-checks
that their `body` opens with `fn` and closes its braces — which it already did — and has no route
from the message to the actual rule (renaming happens via `action="rename"`, not by changing the
name inside a `replace` body).

## Reproduction

Against any Rust file with a test function `foo`:

```
edit_code(action="replace", path="<file>", symbol="foo",
          body="fn bar() { /* same or different body */ }")
```

→ the "dropped the symbol definition" message above, naming `foo` (the old `symbol`) as the
subject, with no mention that `bar` (the name actually in `body`) is why.

Observed live 2026-09-13 while fixing `tests/result_caps.rs`
(`docs/issues/archive/2026-09-03-classify-conflates-two-malformed-reasons-under-one-message.md`): renaming
`unclassified_decls_reports_a_malformed_result_cap_id_under_the_not_a_cap_message` to
`..._with_its_own_message` via `replace` with a full attributes+body payload hit exactly this
message. The fix was to keep `symbol` and the body's fn name identical for the `replace` call, then
issue a separate `action="rename"` call — which worked immediately once tried.

## Environment

codescout MCP server, `edit_code` tool, `action="replace"`. Session date 2026-09-13.

## Root cause

Not read from `edit_code`'s implementation (out of scope for this fix session) — inferred from
behavior: `replace` appears to key its symbol-presence check on parsing the new `body` for a
declaration named `symbol`, so a body naming a *different* function does not "contain" the symbol
being replaced and trips the same guard a truncated/partial body would. The message was written
for the truncated-body case and is reused verbatim for this one, even though the two have
different causes and different remedies.

## Hypotheses tried

1. **Hypothesis:** the `attributes` array was malformed or too short, causing the tool to see an
   incomplete declaration.
   **Test:** re-read the call — `attributes` was a complete list ending in `#[test]`, and `body`
   opened with `fn ... () {` and closed correctly.
   **Verdict:** rejected — the declaration was complete by inspection.
2. **Hypothesis:** `replace` does not support changing the function name in the same call as a
   body edit, and the guard's message is just misattributed to a different (truncation) cause.
   **Test:** retried with the body's fn name matching `symbol` (no rename), which succeeded; then
   issued a separate `action="rename"` call, which also succeeded.
   **Verdict:** confirmed.

## Fix

Not implemented — this file documents the tool's behavior for whoever owns `edit_code`, not a fix
to codescout's own source in this session (which was spent on unrelated repo bugs). Candidate
remedy: when `replace`'s post-parse check finds the body's declared name differs from `symbol`
rather than finding no declaration at all, emit a distinct message naming that mismatch and
pointing at `action="rename"`, instead of reusing the truncated-body wording.

## Tests added

None — this is a report against the codescout tool's own UX, not against this repository's code.

## Workarounds

Do a `replace` with the body's function name unchanged from `symbol`, then a separate
`action="rename"` call for the name change. Two calls, but each succeeds cleanly and the intent
(re-body vs rename) stays legible in the tool-call history.

## Resume

Whoever works on `edit_code`'s guard messages: branch the "declaration not found in body" check
on *why* — no declaration at all (today's message is right) vs. a declaration present but under a
different name (new message, pointing at `rename`).

## References

- Surfaced fixing `docs/issues/archive/2026-09-03-classify-conflates-two-malformed-reasons-under-one-message.md`
  in this same session (2026-09-13).
- `tests/result_caps.rs`'s
  `unclassified_decls_reports_a_malformed_result_cap_id_with_its_own_message` — the function whose
  rename tripped this.
