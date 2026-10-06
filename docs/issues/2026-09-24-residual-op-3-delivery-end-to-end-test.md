---
id: '6c50a7804a8e077c'
kind: bug
status: fixed
title: 'RESIDUAL: Write an end-to-end test driving memory(action=''write'') through call_content and asserting OP-3 delivery (the OP-4 sibling test in record 47 shows it is writable)'
tags:
- cluster/declared-not-wired
closed: 2026-10-06
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Write an end-to-end test driving memory(action='write') through call_content and asserting OP-3 delivery (the OP-4 sibling test in record 47 shows it is writable).

## Parent caveat, verbatim

`docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` (status `fixed`):

> OP-2 is NOT covered and never can be by this mechanism. It declares `Serves: Agent, Task`, which are Claude Code HARNESS tools; codescout has no such tools (verified against the served surface and `ls src/tools/`), so they never enter call_content and no selector_key work can route them. For OP-2 the selector_key gap named as this bug's root cause is not the binding constraint. Of the three triggered rules: OP-3 now routes; OP-4 has its routing precondition only and still cannot fire for an independent reason (see related); OP-2 is structurally unreachable. Also NOT verified end-to-end: the fix is covered by two tests meeting at a verified point — the real tools return Some(key), and route() delivers given a Some selector — but no test drives a real memory(action="write") call through call_content and asserts the OP-3 block appears.

## Fix

The test was written on 2026-09-01, three weeks BEFORE this residual was filed (2026-09-24), in the same commit that inverted `Tool::selector_key`'s default (`30b6fc41`, "invert Tool::selector_key's default so every tool opts in"). The filing was made from the parent's `unverified:` caveat, written at the parent's closing, which the commit then overtook. Re-checked against HEAD on 2026-10-06:

- `src/tools/memory/tests.rs` `a_real_memory_write_call_delivers_op_3` (line 431) builds a real project context (`test_ctx_with_project`), drives `Memory.call_content` with `action=write` (a write that must succeed, so `call_content` reaches the router rather than short-circuiting on the error path), joins the returned text blocks, and asserts the text contains `operator-rule OP-3` and `codescout memory or a tracker` (the rule's body, so a marker with an empty body does not ship green).
- Its doc comment names the mutation that must kill it: `Tool::selector_key`'s default returning `None` again.
- The stub-based sibling `a_triggered_operator_rule_is_delivered_once_per_session` (`src/tools/core/tests.rs`) and `every_registered_tool_supplies_a_selector_key` (`src/server.rs`) point at it as the end-to-end assertion.

No code change in this sweep; the record is bookkeeping. The mutation named in the test's doc comment was NOT re-run in this sweep, and the test itself was not executed here: the claim is that the test exists at HEAD and asserts what the caveat said was missing.

The parent's other two caveat items are unchanged by this: OP-4 has only its routing precondition, and OP-2 (`Serves: Agent, Task`, Claude Code harness tools) is structurally unreachable from codescout.

## Tests added

None in this sweep. The test that closes this record predates it: `a_real_memory_write_call_delivers_op_3` at `src/tools/memory/tests.rs:431`, added by `30b6fc41` (see Fix provenance; the commit's diff adds 66 lines to `src/tools/memory/tests.rs`).

## Fix provenance

- **SHA:** `30b6fc41` (`experiments`)
- **patch-id:** `db34821395d6257eb37562bb779ed9ab4eba091e`

## Resume

Closed on 2026-10-06; nothing to resume. Residual follow-ups, listed and not filed: (1) the test and its mutation (default `selector_key` returning `None`) were not re-run in the closing sweep; the gate covers the test, the mutation probe (`./scripts/mutation-probe.sh`) would cover the kill. (2) OP-3 fires only when the agent already writes through codescout's own memory tool, so a green here does not show the rule reaches the violation it forbids; that scope statement is in the parent and in `every_registered_tool_supplies_a_selector_key`'s doc comment.

## References

- `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md` — parent
